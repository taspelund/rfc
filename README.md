# rfc

A command-line tool to search, retrieve, and display IETF RFCs and Internet-Drafts.

## Features

- **View RFCs and Internet-Drafts** - Fetch and display documents in your preferred viewer
- **Search** - Search the IETF Datatracker for RFCs and drafts by keyword
- **Local caching** - Documents are cached locally for offline access and faster retrieval
- **Flexible viewing** - Open documents in your editor, pager, or any custom program
- **Format conversion** - Automatically converts HTML to plain text when needed

## Installation

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

By default, documents open in your `$EDITOR` (or `less` if not set):

```bash
rfc -p 9000                 # Use $PAGER instead of $EDITOR
rfc -o bat 9000             # Open with a specific program
rfc -o "code -" 9000        # Open in VS Code
```

Open in web browser instead of viewing locally:

```bash
rfc -w 9000                 # Open RFC 9000 on IETF Datatracker
rfc --web draft-ietf-quic-transport  # Open draft in browser
```

### Bypassing Cache

Force a fresh fetch from the network:

```bash
rfc -f 9000                 # Fetch fresh copy, ignoring cache
```

### Searching

Search for RFCs by keyword (default):

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
rfc -s bgp -l 20            # Show only first 20 results
```

### Cache Management

```bash
rfc --cache-info            # Show cache location and size
rfc --list-cache            # List all cached documents
rfc --uncache 9000          # Remove a specific document from cache
rfc --clear-cache           # Clear all cached documents
```

## Configuration

### Viewer Selection

The viewer is selected in the following order:

1. **`-o`/`--open-with`** - If specified, use this program
2. **`-p`/`--pager` flag** - If set, use `$PAGER` environment variable
3. **`$EDITOR`** - Use the editor environment variable
4. **`less`** - Final fallback if nothing else is set

The cache is stored in the platform-specific cache directory:
- **Linux**: `~/.cache/rfc/`
- **macOS**: `~/Library/Caches/rfc/`
- **Windows**: `{FOLDERID_LocalAppData}\rfc\cache\`

## Command Reference

```
Usage: rfc [OPTIONS] [DOCUMENT]

Arguments:
  [DOCUMENT]  RFC number or draft name to view

Options:
  -s, --search <QUERY>      Search for documents
  -p, --pager               Use PAGER instead of EDITOR
  -o, --open-with <PROGRAM> Program to open document with
  -f, --fresh               Fetch fresh copy, ignoring cache
  -w, --web                 Open document in web browser (IETF Datatracker)
  -d, --drafts              Only show drafts (with -s)
  -a, --all                 Show both RFCs and drafts (with -s)
  -l, --limit <N>           Limit search results (with -s)
      --list-cache          List cached documents
      --clear-cache         Clear all cached documents
      --cache-info          Show cache info
      --uncache <DOC>       Remove a document from cache
  -h, --help                Print help
  -V, --version             Print version
```

## Examples

```bash
# Read the QUIC specification
rfc 9000

# Search for TLS-related RFCs
rfc -s tls

# Find the latest HTTP/3 draft
rfc -s http3 -d

# View a document in VS Code
rfc -o "code -" 8446

# Check what's cached
rfc --cache-info
```

## License

BSD-3-Clause
