
### `[package.metadata.deb.systemd-units]` options

When this table is present AND `maintainer-scripts` is specified, correct installation of systemd units will be handled automatically for you.

This works as follows:
1. Assets will be added for any matching systemd unit files found in the `unit-scripts` directory.
2. Code blocks will be generated for enabling, disabling, starting, stopping, and restarting the corresponding systemd services, when the package is installed, updated, or removed.
3. `maintainer-scripts` will be created using the generated code blocks.

**Note:** `<maintainer-scripts>` **MUST** be set, even if it is an empty directory. If non-empty, any maintainer scripts present **MUST** contain the `#DEBHELPER#` token denoting the point at which generated code blocks should be inserted.

The exact behaviour can be tuned using the following options:

 - **unit-scripts**: Directory containing zero or more [systemd unit files](https://www.freedesktop.org/software/systemd/man/systemd.unit.html) (see below for matching rules) (default `maintainer-scripts`).
 - **unit-name**: Only include systemd unit files for this unit (see below for matching rules).
 - **enable**: Enable the systemd unit on package installation and disable it on package removal (default `true`).
 - **start**: Start the systemd unit on package installation and stop it on package removal (default `true`).
 - **restart-after-upgrade**: If true, postpone systemd service restart until after upgrade is complete (+ = less downtime, - = can confuse some programs), otherwise stop the service before upgrade and start it again after upgrade (default `true`).
 - **stop-on-upgrade**: If true stop the systemd on package upgrade and removal, otherwise stop the sytemsd service only on package removal (default `true`).

Systemd unit file names must match one of the following patterns:

 - `<package>.<unit>.<suffix>` - _only if `unit-name` is specified_
 - `<package>.<unit>@.<suffix>` - _only if `unit-name` is specified_
 - `<package>.<suffix>`
 - `<pacakge>@.<suffix>`
 - `<unit>.<suffix>` - _only if `unit-name` is specified_
 - `<unit>@.<suffix>` - _only if `unit-name` is specified_

User supplied `maintainer-scripts` file names must match one of the following patterns:

 - `<package>.<unit>.<script>` - _only if `unit-name` is specified_
 - `<package>.<script>`
 - `<unit>.<script>` - _only if `unit-name` is specified_
 - `<script>`

Where `<script>` is one of: `preinst`, `postinst`, `prerm`, `postrm`.

**NOTE:** When using the variant feature, `<package>` will actually be `<package>-<variant>` unless the variant name has been overridden using `name` in the variant specific metadata table. You can use this to supply variant specific unit files and maintainer scripts.

See:
 - The [systemd documentation](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#Description) for more details on unit naming.
 - The [Debian Policy Manual](https://www.debian.org/doc/debian-policy/ch-maintainerscripts.html) for more information about maintainer scripts.
 - A list of [code blocks](https://github.com/mmstick/cargo-deb/tree/579e10c89b060d=eec05ce8653f501c9eee3a0297/autoscripts) which may be inserted.


Systemd Manager:

```toml
[package.metadata.deb]
maintainer = "Michael Aaron Murphy <mmstickman@gmail.com>"
copyright = "2015-2016, Michael Aaron Murphy <mmstickman@gmail.com>"
license-file = ["LICENSE", "3"]
depends = "$auto, systemd"
extended-description = """\
Written safely in Rust, this systemd manager provides a simple GTK3 GUI interface \
that allows you to enable/disable/start/stop services, monitor service logs, and \
edit unit files without ever using the terminal."""
section = "admin"
priority = "optional"
assets = [
    ["assets/org.freedesktop.policykit.systemd-manager.policy", "usr/share/polkit-1/actions/", "644"],
    ["assets/systemd-manager.desktop", "usr/share/applications/", "644"],
    ["assets/systemd-manager-pkexec", "usr/bin/", "755"],
    ["target/release/systemd-manager", "usr/bin/", "755"]
]
```
