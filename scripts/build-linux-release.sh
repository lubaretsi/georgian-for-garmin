#!/usr/bin/env bash
# Builds the Linux release for this machine's architecture (x86_64 or aarch64): a static (musl)
# command-line tool, and the desktop app as a .deb and an AppImage, built with rustup's Rust and
# the system's GCC and GTK/WebKitGTK, embedding the fonts from `nix build .#fonts`.
#
# Run it outside `nix develop`: binaries from the dev shell link Nix store libraries. Nix only
# builds the fonts and supplies the Tauri CLI, which runs here but is not linked into anything.
#
# The app links the system's glibc, GTK 3 and WebKitGTK 4.1, so it runs on distributions with a
# glibc at least as new as the build machine's (release.yml builds on Ubuntu 22.04: glibc 2.35).
# The .deb installs the udev rule from packaging/linux/; the AppImage can't.
#
# Needs: GCC, binutils, Python 3, file, rustup, Nix with flakes, and the app's build libraries
# (Debian and Ubuntu: libwebkit2gtk-4.1-dev librsvg2-dev xdg-utils). Writes to
# target/linux-release/; the files to publish end up in target/linux-release/dist/. If
# RELEASE_TAG is set (CI on a tag), it must be v<workspace version>.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

die() {
  echo "error: $*" >&2
  exit 1
}
step() { printf '\n==> %s\n' "$*"; }

[[ $(uname -s) == Linux ]] || die "Linux releases are built on Linux"
[[ -z ${IN_NIX_SHELL:-} ]] || die "run this outside \`nix develop\`; its toolchain links Nix store libraries"
nix=$(command -v nix) || die "Nix is needed to build the fonts"
rustup=$(command -v rustup) || die "rustup is needed (https://rustup.rs)"
pkg-config --exists webkit2gtk-4.1 || die "WebKitGTK 4.1's development files are needed (libwebkit2gtk-4.1-dev)"

arch=$(uname -m)
case $arch in
  x86_64) deb_arch=amd64 elf_machine='Advanced Micro Devices X86-64' ;;
  aarch64) deb_arch=arm64 elf_machine=AArch64 ;;
  *) die "unsupported architecture $arch" ;;
esac
gnu_target=$arch-unknown-linux-gnu
musl_target=$arch-unknown-linux-musl
toolchain=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
work=$root/target/linux-release
dist=$work/dist
rule=packaging/linux/70-georgian-for-garmin.rules

step "Checking that rust-toolchain.toml matches nixpkgs"
scripts/check-rust-toolchain.sh

step "Building the fonts and checking them against the pinned hashes"
system=$("$nix" eval --raw --impure --expr builtins.currentSystem)
"$nix" build --no-link --print-build-logs ".#fonts" ".#checks.$system.builder"
fonts=$("$nix" build --no-link --print-out-paths ".#fonts")
cargo_tauri=$("$nix" build --no-link --print-out-paths --inputs-from . nixpkgs#cargo-tauri)/bin/cargo-tauri

step "Installing Rust $toolchain for $gnu_target and $musl_target"
"$rustup" toolchain install "$toolchain" --profile minimal --no-self-update \
  --target "$gnu_target,$musl_target"
rust_bin=$(dirname "$("$rustup" which --toolchain "$toolchain" cargo)")

# The system's toolchain only: nothing from Nix or the caller's compiler settings.
export PATH="$rust_bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
unset CC CXX AR LD CFLAGS CXXFLAGS LDFLAGS RUSTC RUSTC_WRAPPER RUSTFLAGS PKG_CONFIG_PATH \
  CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS CARGO_BUILD_TARGET NIX_CFLAGS_COMPILE NIX_LDFLAGS
export CARGO_TARGET_DIR=$work/cargo
export GEORGIAN_FOR_GARMIN_FONTS=$fonts
# linuxdeploy, which makes the AppImage, is itself an AppImage: run it without FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1

pkgid=$(cargo pkgid --locked -p georgian-for-garmin)
version=${pkgid##*[#@]}
if [[ -n ${RELEASE_TAG:-} && $RELEASE_TAG != "v$version" ]]; then
  die "tag $RELEASE_TAG does not match the workspace version $version"
fi
echo "Georgian for Garmin $version, $arch, $(rustc --version), $(getconf GNU_LIBC_VERSION)"

step "Building the command-line tool"
cargo build --locked --release -p georgian-for-garmin --target "$musl_target"
cli=$CARGO_TARGET_DIR/$musl_target/release/georgian-for-garmin

step "Building the app"
(cd crates/app && "$cargo_tauri" build --ci --target "$gnu_target" --bundles deb,appimage -- --locked)
bundle=$CARGO_TARGET_DIR/$gnu_target/release/bundle
debs=("$bundle"/deb/*.deb) appimages=("$bundle"/appimage/*.AppImage)
[[ ${#debs[@]} == 1 && -f ${debs[0]} ]] || die "expected one .deb in $bundle/deb"
[[ ${#appimages[@]} == 1 && -f ${appimages[0]} ]] || die "expected one AppImage in $bundle/appimage"
deb=${debs[0]} appimage=${appimages[0]}

rm -rf "$work/stage" "$dist"
mkdir -p "$work/stage" "$dist"

step "Checking the packages"
dpkg-deb --extract "$deb" "$work/stage/deb"
[[ $(dpkg-deb --field "$deb" Architecture) == "$deb_arch" ]] || die "$deb is not for $deb_arch"
cmp "$rule" "$work/stage/deb/usr/lib/udev/rules.d/70-georgian-for-garmin.rules" ||
  die "$deb doesn't install $rule"
dpkg-deb --control "$deb" "$work/stage/deb-control"
cmp packaging/linux/deb-postinst.sh "$work/stage/deb-control/postinst" || die "$deb has no postinst from packaging/linux"
cmp packaging/linux/deb-postrm.sh "$work/stage/deb-control/postrm" || die "$deb has no postrm from packaging/linux"
deb_app=$work/stage/deb/usr/bin/georgian-for-garmin-app
(cd "$work/stage" && "$appimage" --appimage-extract >/dev/null)
appimage_app=$work/stage/squashfs-root/usr/bin/georgian-for-garmin-app
[[ -x $deb_app && -x $appimage_app ]] || die "the packages have no georgian-for-garmin-app"

step "Checking the binaries"
check_elf() {
  local bin=$1 machine
  machine=$(readelf --file-header "$bin" | sed -n 's/^ *Machine: *//p')
  [[ $machine == "$elf_machine" ]] || die "$bin is for $machine, not $elf_machine"
}
for bin in "$cli" "$deb_app" "$appimage_app" "$appimage"; do
  check_elf "$bin"
done
cli_headers=$(readelf --program-headers --dynamic "$cli")
[[ $cli_headers != *INTERP* && $cli_headers != *NEEDED* ]] || die "$cli is not statically linked"
echo "georgian-for-garmin: $(file -b "$cli")"
for bin in "$deb_app" "$appimage_app"; do
  glibc=$(objdump --dynamic-syms "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)
  echo "$bin: $(file -b "$bin" | cut -d, -f1-2), needs $glibc"
done
reported=$("$cli" --version)
[[ $reported == *"$version"* ]] || die "$cli --version printed '$reported'"

# Every binary must carry every font byte for byte.
for bin in "$cli" "$deb_app" "$appimage_app"; do
  python3 - "$fonts" "$bin" "$(basename "$bin") ($(realpath --relative-to="$work" "$bin"))" <<'EOF'
import hashlib, json, pathlib, sys

fonts, binary, label = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
data = binary.read_bytes()
manifest = json.loads((fonts / "manifest.json").read_text())
kinds = {pack["id"]: pack["kind"] for pack in manifest["packs"]}
for font in manifest["fonts"]:
    # Bitmap parts by pixel size; TrueType parts scale and have none.
    size = "TrueType" if kinds[font["pack"]] == "truetype" else f"{font['size_px']} px"
    for kind in ("stock", "georgian"):
        blob = (fonts / font[kind]).read_bytes()
        offset = data.find(blob)
        if offset < 0:
            sys.exit(f"error: {label} does not embed {font[kind]}")
        embedded = hashlib.sha256(data[offset:offset + len(blob)]).hexdigest()
        if kind == "georgian":
            print(f"{label}: embeds {font[kind]} ({size}) sha256 {embedded}")
EOF
done

step "Packaging"
# No version in the file names, so the README's releases/latest/download/<name> links keep working.
cli_name=georgian-for-garmin-linux-$arch
deb_name=Georgian-for-Garmin-linux-$arch.deb
appimage_name=Georgian-for-Garmin-linux-$arch.AppImage
mkdir "$work/stage/$cli_name"
install -m 755 "$cli" "$work/stage/$cli_name/georgian-for-garmin"
install -m 644 LICENSE "$work/stage/$cli_name/LICENSE"
install -m 644 "$fonts/LICENSES/OFL.txt" "$work/stage/$cli_name/OFL.txt"
install -m 644 "$rule" "$work/stage/$cli_name/$(basename "$rule")"
tar -czf "$dist/$cli_name.tar.gz" --owner 0 --group 0 --numeric-owner -C "$work/stage" "$cli_name"
install -m 644 "$deb" "$dist/$deb_name"
install -m 755 "$appimage" "$dist/$appimage_name"
install -m 644 "$fonts/manifest.json" "$dist/manifest.json"
(cd "$dist" && sha256sum "$cli_name.tar.gz" "$deb_name" "$appimage_name" manifest.json >SHA256SUMS)
rm -rf "$work/stage"

step "Done: $dist"
ls -l "$dist"
cat "$dist/SHA256SUMS"
