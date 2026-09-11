# kryuk

dead simple bootstrapper for ```org.vinegarhq.sober.```

## installation

```bash
cargo build --release
cargo install --path .
ln -sf ~/.cargo/bin/kryuk ~/.local/bin/kryuk
```

## usage

```bash
# edit fastflags
kryuk edit

# run sober with fastflags applied
kryuk run

# run with launch arguments
kryuk run roblox://placeid=123456

# show fastflags
kryuk status
```

## fastflags file

location: `~/.config/kryuk/fflags.json`

```json
{
  "FFlagDebugGraphicsPreferOpenGL": true
}
```
