# SYNAPSE_ Compatibility

What SYNAPSE_ supports across platforms, terminal escape sequences, and graphics
protocols. Terminal emulation is backed by `alacritty_terminal` (VT100/xterm core).

---

## Platforms

| Platform | GPU backend | Status |
|----------|-------------|--------|
| macOS 11+ | Metal | ✅ Supported |
| Linux X11 | Vulkan / GLES 3.1 | ✅ Supported |
| Linux Wayland | Vulkan / GLES 3.1 | ✅ Supported (client-side decorations) |
| Raspberry Pi 4 / 5 | Mesa V3D (GLES 3.1) | ✅ Supported (effects auto-disabled) |
| Raspberry Pi 3 and older | VideoCore IV (GLES 2.0) | ❌ Unsupported (wgpu needs GLES 3.1) |
| Windows | — | ❌ Not supported |

The renderer auto-detects downlevel adapters (no compute shaders) and switches to
`Limits::downlevel_defaults()`, clamps the glyph atlas to the device's max texture
size, and disables post-processing effects.

---

## `$TERM`

SYNAPSE_ identifies as `xterm-256color`. True-color (24-bit) is fully supported.

---

## Escape sequences

### CSI / SGR
- 16 / 256 / 24-bit color, bold, dim, italic, underline (incl. curly/colored), strikethrough, reverse, blink.
- Cursor styles via `DECSCUSR` (block / bar / underline, steady & blinking).
- `DECRQM` mode queries (tmux / vim detect feature support correctly).
- Mouse reporting: X10, normal, button-event, any-event; SGR (1006) extended coordinates.

### OSC
| Sequence | Purpose | Status |
|----------|---------|--------|
| OSC 0/2 | Window / tab title | ✅ |
| OSC 7 | Working-directory reporting | ✅ |
| OSC 8 | Hyperlinks | ✅ |
| OSC 52 | Clipboard read/write (incl. remote) | ✅ |
| OSC 9 / 777 | Desktop notifications | ✅ |
| OSC 133 | Semantic prompt marks (A/B/C/D) | ✅ |
| OSC 4 / 10 / 11 | Palette / fg / bg color set | ✅ |

### Kitty keyboard protocol (KKP)
Progressive enhancement query / push / pop (`CSI ? u`, `CSI = N u`, `CSI < N u`) supported.

---

## Graphics protocols

| Protocol | Status |
|----------|--------|
| **Sixel** | ✅ Native decoder |
| **iTerm2 inline images** (OSC 1337) | ✅ Incl. animated GIF / APNG playback |
| **Kitty graphics** (APC `_G`) | ✅ See matrix below |

### Kitty graphics — supported keys

| Capability | Keys | Status |
|------------|------|--------|
| Actions | `a=T` transmit, `a=t` transmit-and-put, `a=p` put, `a=q` query, `a=d` delete | ✅ |
| Formats | `f=24` RGB, `f=32` RGBA, `f=100` PNG | ✅ |
| Chunked transmission | `m=1` / `m=0` (multi-chunk) | ✅ |
| Compression | `o=z` (zlib) | ✅ |
| Transmission medium | `t=d` direct, `t=f` file, `t=t` temp file | ✅ |
| Query response | feature detection replies (`OK` / error, honours `q=`) | ✅ |
| Shared memory | `t=s` | ❌ Skipped |
| Unicode placeholders | `U=1` | ❌ Parsed, not rendered |

`kitten icat` works for direct, file, chunked and compressed transmits.

---

## Text & internationalization

| Feature | Status |
|---------|--------|
| Unicode + grapheme clusters | ✅ |
| Programming ligatures (rustybuzz) | ✅ |
| Color emoji (CBDT/CBLC) | ✅ |
| Font fallback chain | ✅ |
| **BiDi / RTL** (Arabic, Hebrew) | ✅ Visual reordering (UAX #9) + Arabic joining¹ |
| CJK wide characters | ✅ |

¹ RTL shaping is in place (gated on RTL codepoints so LTR text keeps the fast path).
Rendering Arabic/Hebrew glyphs requires an RTL-capable font in `font_family`, since
the bundled JetBrains Mono has no Arabic/Hebrew glyphs. Logical order is used for
selection and the cursor.

---

## Known limitations
- Kitty `t=s` (shared memory) and `U=1` (unicode placeholders) are not implemented.
- BiDi selection/copy uses logical order (no visual-order selection).
- Windows is not supported.
