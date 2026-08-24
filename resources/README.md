# resources

This directory is required at compile time by `maverick_os::start!` / `ramp::run!`,
which embeds it with `include_dir!("$CARGO_MANIFEST_DIR/resources")`.

The `KnightsOfQuartz` game loads its art/sound from `assets/` via `include_bytes!`,
so this directory is intentionally empty for the windowed build. The headless
simulation driver (`bin/headless`) does not use it either.
