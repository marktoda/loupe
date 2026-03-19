# loupe

TUI viewer for Claude Code JSONL streams.

## Install

```bash
cargo install --path .
```

## Usage

```bash
loupe <directory>
```

Point at a directory containing `.jsonl` files from `claude --output-format stream-json`.

## Keybindings

| Key | Action |
|-----|--------|
| `q` / `Ctrl-c` | Quit |
| `Tab` | Switch pane focus |
| `1` / `2` / `3` | Transcript / Tools / Raw view |
| `j` / `k` | Scroll / Select |
| `g` / `G` | Top / Bottom |
| `/` | Search |
| `n` / `N` | Next / Previous match |
| `Enter` | Expand tool detail |
| `f` | Follow live run |
| `?` | Help |
