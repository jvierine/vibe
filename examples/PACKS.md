# Browse Vibe programs

These generated pages expose the actual stored functions, signatures and call relationships. The binary packs remain authoritative.

- [fft1024](fft1024.vibepack.md)
- [hello_world](hello_world.vibepack.md)
- [metablate](metablate/metablate.vibepack.md)

Build the compiler first: `cargo build`.
Regenerate: `conda run -n base python scripts/render_packs.py`.
Check freshness without writing: `conda run -n base python scripts/render_packs.py --check`.
