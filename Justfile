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
# All six of upstream's, at upstream's paths, plus this fork's two. HyprCosmic
# is a fork of COSMIC rather than something installed next to it, so a machine
# that installs this gets its cosmic-session from here and there is nothing else
# on disk to provide the target, the mimeapps list or the dconf profile.
#
# The two extra files are what make it a HyprCosmic install rather than a rebuilt
# COSMIC one:
#
#   data/start-hyprcosmic  -> /usr/bin/start-hyprcosmic
#   data/hyprcosmic.desktop -> a second entry on the greeter's menu
#
# Both session entries are installed, and both are served by these same
# binaries. That is deliberate. Replacing COSMIC does not have to mean removing
# its shell, and keeping cosmic.desktop costs one file while preserving the one
# property worth keeping from the old beside-install layout: if the HyDE shell
# will not start, there is still something on the greeter's menu that will.
install:
    # main binary
    install -Dm0755 {{ cargo-target-dir }}/release/cosmic-session {{ bindir }}/cosmic-session

    # session start scripts, this fork's and upstream's
    install -Dm0755 data/start-hyprcosmic {{ bindir }}/start-hyprcosmic
    install -Dm0755 data/start-cosmic {{ bindir }}/start-cosmic

    # Upstream rewrites DCONF_PROFILE in its own start script to an absolute
    # path under the build prefix. That is carried across for start-cosmic,
    # because it is upstream's file and diverging from it here would be a change
    # this fork has no reason to make.
    #
    # start-hyprcosmic is deliberately NOT sed'd. It searches /etc/dconf/profile
    # and then XDG_DATA_DIRS and exports the bare profile name only if it finds
    # one, which keeps an administrator's /etc override working; pinning a build
    # prefix into it would break that. The script says so where it searches.
    sed -i "s|DCONF_PROFILE=cosmic|DCONF_PROFILE={{ cosmic_dconf_profile }}|" {{ bindir }}/start-cosmic

    # systemd user target
    install -Dm0644 data/cosmic-session.target {{ systemddir }}/cosmic-session.target

    # session entries. Note that both .desktop files name an absolute Exec under
    # /usr/bin: a .desktop has no way to interpolate a prefix, so building with
    # prefix != /usr installs entries pointing at paths this recipe did not write.
    install -Dm0644 data/hyprcosmic.desktop {{ sessiondir }}/hyprcosmic.desktop
    install -Dm0644 data/cosmic.desktop {{ sessiondir }}/cosmic.desktop

    # default applications
    install -Dm0644 data/cosmic-mimeapps.list {{ applicationdir }}/cosmic-mimeapps.list

    # dconf profile
    install -Dm0644 data/dconf/profile/cosmic {{ rootdir }}/{{ cosmic_dconf_profile }}

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
