# rfc

A command-line tool to search, retrieve, and display IETF RFCs and Internet-Drafts.

## Features

- **View RFCs and Internet-Drafts** - Fetch and display documents in your preferred viewer
- **Search** - Search the IETF Datatracker for RFCs and drafts by keyword
- **Local caching** - Documents are cached locally for offline access and faster retrieval
- **Flexible viewing** - Open documents in your editor, pager, or any custom program
- **Format conversion** - Automatically converts HTML to plain text when needed

## Installation

This tool requires Rust. If you don't have Rust installed, visit [rustup.rs](https://rustup.rs/) for installation instructions.

Build and install from source using Cargo:

```bash
git clone https://github.com/your-username/rfc.git
cd rfc
cargo install --path .
```

## Usage

### Viewing Documents

View an RFC by number:

```bash
rfc 9000                    # View RFC 9000 (QUIC)
rfc rfc9000                 # Also works with "rfc" prefix
rfc RFC9000                 # Case insensitive
```

View an Internet-Draft:

```bash
rfc draft-ietf-quic-transport       # Latest version auto-resolved
rfc draft-ietf-quic-transport-34    # Specific version
```

### Viewer Options

By default, documents open in your `$EDITOR` (or `$PAGER` if EDITOR is not set):

```bash
rfc 9000                    # Open with $EDITOR or $PAGER
rfc -o /usr/bin/less 9000   # Open with specific program
rfc -o "code -" 9000        # Open in VS Code
rfc -f 9000                 # Fetch and cache only, don't open
```

### Cache Control

- **No flag** (default): Use cache if available, fetch from API if not
- **`-r`/`--refresh`**: Fetch from API (ignoring cache) then open
- **`-f`/`--fetch-only`**: Fetch from API (ignoring cache) and cache only, don't open

```bash
rfc 9000                    # Use cache if available
rfc -r 9000                 # Fetch fresh from API and open
rfc -f 9000                 # Fetch fresh from API and cache (no opening)
```

### Searching

Search for RFCs by keyword (default), results show titles for easy identification:

```bash
rfc -s quic                 # Search for RFCs containing "quic"
rfc -s "congestion control" # Search with multiple words
```

Search for drafts or both:

```bash
rfc -s quic -d              # Search only Internet-Drafts
rfc -s quic -a              # Search both RFCs and drafts
```

Limit results:

```bash
rfc -s bgp -l 20            # Show first 20 results
```

**Note:** Search results display document names in a copy-paste friendly format (e.g., `rfc4271`) that you can use directly with `rfc rfc4271`.

### Cache Management

```bash
rfc --cache-info            # Show cache location and size
rfc --list-cache            # List all cached documents with titles
rfc --list-cache -w         # List cache with full titles (no truncation)
rfc --uncache 9000          # Remove a specific document from cache
rfc --clear-cache           # Clear all cached documents
```

The `--list-cache` display shows document titles for easy identification, with titles truncated to fit within 80 characters per line (or full width with `-w` flag). Document names are displayed in copy-paste friendly format (e.g., `rfc4271`) for use with `rfc rfc4271`.

## Configuration

### Viewer Selection

By default, documents open with `$EDITOR` if available, otherwise `$PAGER`:

- **`rfc 9000`** - Use `$EDITOR` if set, else `$PAGER`, else don't open
- **`rfc 9000 -o PROGRAM`** - Use the specified program (e.g., `less`, `bat`, `code -`)

### Fetch Behavior

Control when documents are fetched from the API:

- **`rfc 9000`** - Use cache if available, otherwise fetch from API
- **`-r`/`--refresh`** - Fetch from API, ignore cache (implies fresh fetch)
- **`-f`/`--fetch-only`** - Fetch from API and cache only, don't open (implies fresh fetch)

Both `-r` and `-f` skip the local cache and always fetch from the API. The difference is that `-r` opens the document after fetching, while `-f` just caches it.

### Cache Storage

The cache is stored in the platform-specific cache directory:
- **Linux**: `~/.cache/rfc/`
- **macOS**: `~/Library/Caches/rfc/`
- **Windows**: `{FOLDERID_LocalAppData}\rfc\cache\`

Cached documents include both the document content (`.txt`) and metadata with titles (`.meta`).

## Command Reference

```
Usage: rfc [OPTIONS] [DOCUMENT]

Arguments:
  [DOCUMENT]  RFC number or draft name to view

Options:
  -s, --search <QUERY>       Search for documents
  -o, --open-with <PROGRAM>  Program to open document with
  -f, --fetch-only           Fetch from API only, skip cache
  -r, --refresh              Refresh from API, ignore cache (then open)
  -d, --drafts               Only show drafts (with -s)
  -a, --all                  Show both RFCs and drafts (with -s)
  -l, --limit <LIMIT>        Limit search results (with -s)
  -w, --wide                 Show full titles without truncation (with --list-cache)
      --list-cache           List cached documents
      --clear-cache          Clear all cached documents
      --cache-info           Show cache info
      --uncache <DOC>        Remove a document from cache
  -h, --help                 Print help
  -V, --version              Print version
```

## Examples

```bash
# Read the QUIC specification (uses cache if available)
rfc 9000

# Fetch fresh copy from API
rfc -r 9000

# Fetch and cache without opening
rfc -f 9000

# Search for TLS-related RFCs
rfc -s tls

# Find the latest HTTP/3 draft
rfc -s http3 -d

# View a document with a specific program
rfc -o /usr/bin/less 8446
rfc -o "code -" 8446

# Check what's cached (with full titles)
rfc --cache-info
rfc --list-cache -w
```

## License

BSD-3-Clause
