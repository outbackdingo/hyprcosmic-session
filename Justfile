rootdir := ''
prefix := '/usr'
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
orca := '/usr/bin/orca'
cosmic_dconf_profile := prefix + '/share/dconf/profile/cosmic'
usrdir := absolute_path(clean(rootdir / prefix))
bindir := usrdir / 'bin'
systemddir := usrdir / 'lib' / 'systemd' / 'user'
sessiondir := usrdir / 'share' / 'wayland-sessions'
applicationdir := usrdir / 'share' / 'applications'

# The fork's own binary directory. It stays out of bindir for the same reason
# the compositor does: the distro's cosmic-session owns /usr/bin/cosmic-session,
# and overwriting it would break the stock session -- the one you need to log
# into when this one will not start.
privdir := usrdir / 'libexec' / 'hyprcosmic'

default: build-release

build-debug *args:
    ORCA={{ orca }} cargo build {{ args }}

# Compile with release profile
build-release *args: (build-debug '--release' args)

# Compile with a vendored tarball
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Remove Cargo build artifacts
clean:
    cargo clean

# Also remove .cargo and vendored dependencies
clean-dist: clean
    rm -rf .cargo vendor vendor.tar target

# Installs files into the system
#
# Three files, where upstream installs seven. The four that are gone are not
# oversights -- each is owned by the distro's own cosmic-session package, and
# writing them from here would make a HyprCosmic package conflict with it:
#
#   data/start-cosmic          -> /usr/bin/start-cosmic
#   data/cosmic-session.target -> systemd user target
#   data/cosmic-mimeapps.list  -> the default-applications list
#   data/dconf/profile/cosmic  -> the dconf profile
#
# None of them are things this fork changes, and a HyprCosmic session needs
# stock COSMIC installed regardless, for the greeter, the portals and the
# settings daemon. So it reads that package's copies rather than shipping rival
# ones. `data/cosmic.desktop` is dropped for the same reason and replaced by
# hyprcosmic.desktop, which is what puts the second entry on the greeter's menu
# instead of overwriting the first.
#
# Upstream's `sed` over DCONF_PROFILE is deliberately not carried across.
# start-hyprcosmic searches /etc/dconf/profile and then XDG_DATA_DIRS for the
# profile and exports the bare name only if it finds one; rewriting a prefix
# into it would pin one location and break an administrator's /etc override.
# The script says so at the point where it does the search.
install:
    # main binary
    install -Dm0755 {{ cargo-target-dir }}/release/cosmic-session {{ privdir }}/cosmic-session

    # session start script
    install -Dm0755 data/start-hyprcosmic {{ bindir }}/start-hyprcosmic

    # session entry. Note that hyprcosmic.desktop names /usr/bin/start-hyprcosmic
    # as an absolute Exec: a .desktop file has no way to interpolate a prefix,
    # so building with prefix != /usr installs an entry that points at a path
    # this recipe did not write.
    install -Dm0644 data/hyprcosmic.desktop {{ sessiondir }}/hyprcosmic.desktop

# Vendor Cargo dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    tar pcf vendor.tar vendor
    rm -rf vendor

# Extracts vendored dependencies
[private]
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{ version }}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{ version }}'
    git commit --amend
    git tag -a {{ version }} -m ''
