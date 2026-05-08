# rfc

A command-line tool to search, retrieve, and display IETF RFCs and Internet-Drafts.

## Features

- **View RFCs and Internet-Drafts** - fetch and display documents in your preferred viewer
- **Search** - query the IETF Datatracker by keyword
- **Local caching** - documents are cached for offline access and faster repeat reads
- **Format conversion** - falls back to HTML and converts to plain text when no .txt is published

## Installation

Requires Rust. If you don't have it, see [rustup.rs](https://rustup.rs/).

```bash
git clone https://github.com/your-username/rfc.git
cd rfc
cargo install --path .
```

## Usage

The default command, `rfc <document>`, looks in the local cache first and only hits the network on a miss. Everything else lives under a subcommand.

### View a document

```bash
rfc 9000                    # RFC 9000 (QUIC)
rfc rfc9000                 # works with prefix
rfc RFC 9000                # case-insensitive, space tolerated
rfc draft-ietf-quic-transport       # latest draft version auto-resolved
rfc draft-ietf-quic-transport-34    # pinned version
```

### Pick a viewer

By default the document opens in `$EDITOR`, then `$PAGER`, then nothing. Override with `-o`:

```bash
rfc -o less 9000
rfc -o "code -" 9000        # quoted forms with arguments are split for you
```

The document is written to a tempfile and the viewer is invoked with the path as its final argument - works for editors and pagers alike.

### Refresh from the API

There's no `--refresh` flag. To force a re-fetch, run `rfc fetch` then `rfc <doc>`:

```bash
rfc fetch 9000              # always hits the API, caches, doesn't open
rfc 9000                    # now reads from the freshened cache
```

### Search

```bash
rfc search quic
rfc search bgp message              # multi-token, word-order independent
rfc search quic -d                  # drafts only
rfc search quic -a                  # both RFCs and drafts (default: RFCs only)
rfc search bgp -l 50                # raise the result cap (default 25)
```

### Cache management

```bash
rfc cache list              # cached documents with titles
rfc cache list -w           # don't truncate titles
rfc cache info              # location + total size
rfc cache remove 9000       # drop a single document
rfc cache clear             # nuke everything
```

### Cache location

- Linux: `~/.cache/rfc/`
- macOS: `~/Library/Caches/rfc/`
- Windows: `{FOLDERID_LocalAppData}\rfc\cache\`

Each document is stored with its content (`.txt`) and a metadata sidecar (`.meta`) holding the title.

## License

BSD-3-Clause
