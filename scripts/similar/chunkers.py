"""Per-corpus chunking strategies for `just similar`.

Each chunker yields `Chunk` records from a single source file. The
chunker chosen per file is decided by `chunk_path` based on the file's
location in the repo.

Chunk-id shape: `<source_kind>/<source_stem>:<section>` for prose
sections, `<source_kind>/<source_stem>:<item_kind>:<item_name>` for
Rust doc-comments. Stable across rebuilds — the retrieval envelope
relies on these as the primary `id` field.

Section text always begins with a synthetic context header (e.g.
`ticket 189 (status: done) — ## Why`) so that embedding captures the
chunk's location, not just its raw content. This is the same trick
used in retrieval-augmented systems to keep chunks self-describing.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any, Iterator


# Approximate token budget per chunk before sub-splitting kicks in.
# 400 tokens ≈ 300 words ≈ 1500-2000 chars for English prose.
SUB_CHUNK_THRESHOLD_CHARS = 2000


@dataclass
class Chunk:
    chunk_id: str
    source_path: str          # repo-relative
    source_kind: str          # tickets | landed | balance | pre-existing | systems | dses | planner | markers
    section: str | None
    text: str                 # what gets embedded
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


# ── frontmatter + section parsing ───────────────────────────────────────────

_FRONTMATTER_RE = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)
_SECTION_RE = re.compile(r"^##\s+(.+?)\s*$", re.MULTILINE)


def _parse_frontmatter(text: str) -> tuple[dict[str, Any], str]:
    """Pull a YAML-ish frontmatter block off the top of `text`.

    Doesn't pull in PyYAML — the frontmatter shape across tickets is
    flat key: value (with `[]`, `null`, ints, dates), and we only need
    a few keys for metadata. Anything fancy gets stored as the raw
    string. Returns `(metadata_dict, body_after_frontmatter)`."""
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return {}, text
    raw = m.group(1)
    body = text[m.end():]
    meta: dict[str, Any] = {}
    for line in raw.splitlines():
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        meta[key.strip()] = _coerce_scalar(value.strip())
    return meta, body


def _coerce_scalar(raw: str) -> Any:
    if raw in ("null", "", "~"):
        return None
    if raw == "true":
        return True
    if raw == "false":
        return False
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        if not inner:
            return []
        return [_coerce_scalar(p.strip()) for p in inner.split(",")]
    try:
        return int(raw)
    except ValueError:
        pass
    try:
        return float(raw)
    except ValueError:
        pass
    return raw


def _split_sections(body: str) -> list[tuple[str, str]]:
    """Walk a markdown body and yield (heading, section_body) pairs.

    Anything before the first `## ` heading becomes a section called
    `_preamble` so it isn't dropped (some tickets have prose before
    the first heading)."""
    sections: list[tuple[str, str]] = []
    matches = list(_SECTION_RE.finditer(body))
    if not matches:
        if body.strip():
            sections.append(("_preamble", body.strip()))
        return sections
    if matches[0].start() > 0:
        preamble = body[:matches[0].start()].strip()
        if preamble:
            sections.append(("_preamble", preamble))
    for i, m in enumerate(matches):
        heading = m.group(1).strip()
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(body)
        section_body = body[start:end].strip()
        if section_body:
            sections.append((heading, section_body))
    return sections


def _maybe_split_long_section(
    heading: str,
    section_body: str,
) -> list[tuple[str, str]]:
    """Sub-chunk a section if it exceeds `SUB_CHUNK_THRESHOLD_CHARS`.

    Splits on blank lines (paragraph boundaries), greedily packing
    paragraphs into ~threshold-sized buckets. Each sub-chunk inherits
    the parent heading with a `:partN` suffix."""
    if len(section_body) <= SUB_CHUNK_THRESHOLD_CHARS:
        return [(heading, section_body)]
    paragraphs = [p.strip() for p in re.split(r"\n\s*\n", section_body) if p.strip()]
    buckets: list[list[str]] = [[]]
    cur_len = 0
    for para in paragraphs:
        if cur_len + len(para) > SUB_CHUNK_THRESHOLD_CHARS and buckets[-1]:
            buckets.append([])
            cur_len = 0
        buckets[-1].append(para)
        cur_len += len(para) + 2
    return [
        (f"{heading}:part{i+1}", "\n\n".join(b))
        for i, b in enumerate(buckets) if b
    ]


# ── chunkers ────────────────────────────────────────────────────────────────

def chunk_ticket_or_landed(
    path: Path,
    repo_root: Path,
    source_kind: str,    # "tickets" or "landed"
) -> Iterator[Chunk]:
    """Section-window chunker for tickets and landed.

    Frontmatter becomes per-chunk metadata. Each `## Heading` becomes a
    chunk; the synthetic header line at the top of `text` carries the
    ticket number + status + cluster + heading so retrieval can show
    context without re-parsing frontmatter on every hit.
    """
    text = path.read_text(encoding="utf-8")
    meta, body = _parse_frontmatter(text)
    stem = path.stem
    rel_path = str(path.relative_to(repo_root))
    ticket_id = meta.get("id", stem)
    status = meta.get("status", "?")
    cluster = meta.get("cluster") or "—"
    initiative = meta.get("initiative") or []
    landed_on = meta.get("landed-on")
    title = meta.get("title", "")

    header_bits = [
        f"ticket {ticket_id}",
        f"status: {status}",
        f"cluster: {cluster}",
    ]
    if initiative:
        header_bits.append(f"initiative: {', '.join(initiative)}")
    if landed_on:
        header_bits.append(f"landed: {landed_on}")
    if title:
        header_bits.append(f"title: {title}")
    header_line = " · ".join(header_bits)

    for heading, section_body in _split_sections(body):
        for sub_heading, sub_body in _maybe_split_long_section(heading, section_body):
            embed_text = f"{header_line}\n## {sub_heading}\n\n{sub_body}"
            yield Chunk(
                chunk_id=f"{source_kind}/{stem}:{sub_heading}",
                source_path=rel_path,
                source_kind=source_kind,
                section=sub_heading,
                text=embed_text,
                metadata={
                    "ticket_id": ticket_id,
                    "title": title,
                    "status": status,
                    "cluster": cluster,
                    "initiative": initiative,
                    "landed_on": landed_on,
                },
            )


def chunk_balance_or_systems(
    path: Path,
    repo_root: Path,
    source_kind: str,    # "balance" or "systems"
) -> Iterator[Chunk]:
    """Heading-driven chunker without frontmatter.

    Balance and system docs share the same shape (no frontmatter,
    `##` heading-driven). The synthetic header line carries the
    document filename so retrieval can attribute hits to a thread."""
    text = path.read_text(encoding="utf-8")
    rel_path = str(path.relative_to(repo_root))
    stem = path.stem
    title = _extract_h1(text) or stem
    header_line = f"{source_kind}/{stem} — {title}"

    for heading, section_body in _split_sections(text):
        for sub_heading, sub_body in _maybe_split_long_section(heading, section_body):
            embed_text = f"{header_line}\n## {sub_heading}\n\n{sub_body}"
            yield Chunk(
                chunk_id=f"{source_kind}/{stem}:{sub_heading}",
                source_path=rel_path,
                source_kind=source_kind,
                section=sub_heading,
                text=embed_text,
                metadata={"doc_title": title},
            )


def _extract_h1(text: str) -> str | None:
    """Pull the first `# Heading` line if present."""
    m = re.search(r"^#\s+(.+?)\s*$", text, re.MULTILINE)
    return m.group(1).strip() if m else None


def chunk_whole_file(
    path: Path,
    repo_root: Path,
    source_kind: str,    # "pre-existing"
) -> Iterator[Chunk]:
    """Whole-file chunker. For pre-existing/, where files are short and
    freeform — section-windowing would either drop preamble or split
    awkwardly across freeform headings."""
    text = path.read_text(encoding="utf-8")
    rel_path = str(path.relative_to(repo_root))
    stem = path.stem
    title = _extract_h1(text) or stem
    header_line = f"{source_kind}/{stem} — {title}"
    for sub_heading, sub_body in _maybe_split_long_section("_full", text.strip()):
        embed_text = f"{header_line}\n\n{sub_body}"
        yield Chunk(
            chunk_id=f"{source_kind}/{stem}:{sub_heading}",
            source_path=rel_path,
            source_kind=source_kind,
            section=sub_heading if sub_heading != "_full" else None,
            text=embed_text,
            metadata={"doc_title": title},
        )


# ── Rust doc-comment chunker ────────────────────────────────────────────────

# Match a contiguous block of `///` line-doc-comments (item-level docs).
_ITEM_DOC_RE = re.compile(
    r"^((?:[ \t]*///[^\n]*\n)+)([ \t]*(?:pub(?:\([^)]*\))?\s+)?"
    r"(?:async\s+)?(fn|struct|enum|trait|impl|mod|const|static|type|union|macro_rules!)\b[^\n;{]*)",
    re.MULTILINE,
)
_INNER_DOC_RE = re.compile(r"^[ \t]*//![ \t]?(.*)$", re.MULTILINE)
_DOC_LINE_RE = re.compile(r"^[ \t]*///[ \t]?(.*)$")


def chunk_rust_doc_comments(
    path: Path,
    repo_root: Path,
    source_kind: str,    # "dses" | "planner" | "markers"
) -> Iterator[Chunk]:
    """Extract `///` and `//!` doc-comments from a Rust file, one chunk
    per documented item.

    Module-level `//!` becomes a single chunk with item kind `mod!`.
    Each contiguous `///` block followed by a declaration becomes one
    chunk; the declaration's first line is included verbatim so the
    chunk text says what was documented (e.g. `pub fn score_consideration`).
    Items without doc-comments are skipped — we're embedding *prose*,
    not the whole symbol surface.
    """
    text = path.read_text(encoding="utf-8")
    rel_path = str(path.relative_to(repo_root))
    stem = path.stem

    inner_lines = _INNER_DOC_RE.findall(text)
    if inner_lines:
        body = "\n".join(line.strip() for line in inner_lines if line.strip())
        if body:
            embed_text = f"{source_kind}/{stem} (module docs)\n\n{body}"
            yield Chunk(
                chunk_id=f"{source_kind}/{stem}:mod!",
                source_path=rel_path,
                source_kind=source_kind,
                section="mod!",
                text=embed_text,
                metadata={"item_kind": "mod"},
            )

    for m in _ITEM_DOC_RE.finditer(text):
        doc_block = m.group(1)
        decl_first_line = m.group(2).strip()
        item_kind = m.group(3)

        doc_lines = []
        for line in doc_block.splitlines():
            dm = _DOC_LINE_RE.match(line)
            if dm:
                doc_lines.append(dm.group(1))
        prose = "\n".join(doc_lines).strip()
        if not prose:
            continue

        item_name = _extract_item_name(decl_first_line, item_kind)
        embed_text = (
            f"{source_kind}/{stem} — {item_kind} {item_name}\n"
            f"```rust\n{decl_first_line}\n```\n\n"
            f"{prose}"
        )
        yield Chunk(
            chunk_id=f"{source_kind}/{stem}:{item_kind}:{item_name}",
            source_path=rel_path,
            source_kind=source_kind,
            section=f"{item_kind} {item_name}",
            text=embed_text,
            metadata={"item_kind": item_kind, "item_name": item_name},
        )


def _extract_item_name(decl: str, kind: str) -> str:
    """Pull the identifier after `fn`/`struct`/`impl`/etc. Best-effort —
    on impl blocks the 'name' is the type being implemented for, which
    is not always a single token; we return whatever's between `impl`
    and the first `{`/`for` to keep the chunk_id stable."""
    if kind == "impl":
        # `impl Foo` → "Foo"; `impl Trait for Foo` → "Trait for Foo".
        m = re.match(r"^.*?\bimpl\b\s*(?:<[^>]*>\s*)?(.+?)(?:\s*\{|$)", decl)
        return m.group(1).strip().replace(" ", "_") if m else "anon"
    if kind == "macro_rules!":
        m = re.match(r"^macro_rules!\s+(\w+)", decl)
        return m.group(1) if m else "anon"
    m = re.match(rf"^.*?\b{re.escape(kind)}\b\s+(\w+)", decl)
    return m.group(1) if m else "anon"


# ── dispatcher ──────────────────────────────────────────────────────────────

# Mapping repo-relative path prefix → (source_kind, chunker_fn).
# Order matters: more specific prefixes come first.
_DISPATCH: list[tuple[str, str, Any]] = [
    ("docs/open-work/tickets/",      "tickets",      "ticket_or_landed"),
    ("docs/open-work/landed/",       "landed",       "ticket_or_landed"),
    ("docs/open-work/pre-existing/", "pre-existing", "whole_file"),
    ("docs/balance/",                "balance",      "balance_or_systems"),
    ("docs/systems/",                "systems",      "balance_or_systems"),
    ("src/ai/dses/",                 "dses",         "rust_doc_comments"),
    ("src/ai/planner/",              "planner",      "rust_doc_comments"),
    ("src/components/markers.rs",    "markers",      "rust_doc_comments"),
]


def chunker_for(rel_path: str) -> tuple[str, str] | None:
    """Return `(source_kind, chunker_id)` for a repo-relative path, or
    None if the path isn't covered by any chunker. Used by the index
    builder to decide whether to walk a file at all."""
    for prefix, kind, chunker_id in _DISPATCH:
        if rel_path.startswith(prefix):
            return kind, chunker_id
    return None


def chunk_path(path: Path, repo_root: Path) -> list[Chunk]:
    """Dispatch a single file to its chunker. Returns an empty list if
    the path isn't covered or the file is empty."""
    rel_path = str(path.relative_to(repo_root))
    routing = chunker_for(rel_path)
    if routing is None:
        return []
    kind, chunker_id = routing
    if not path.exists() or path.stat().st_size == 0:
        return []
    if chunker_id == "ticket_or_landed":
        return list(chunk_ticket_or_landed(path, repo_root, kind))
    if chunker_id == "balance_or_systems":
        return list(chunk_balance_or_systems(path, repo_root, kind))
    if chunker_id == "whole_file":
        return list(chunk_whole_file(path, repo_root, kind))
    if chunker_id == "rust_doc_comments":
        return list(chunk_rust_doc_comments(path, repo_root, kind))
    raise RuntimeError(f"unknown chunker_id: {chunker_id}")


# ── corpus discovery ────────────────────────────────────────────────────────

def discover_corpus_files(repo_root: Path) -> list[Path]:
    """Walk the configured corpus prefixes and return every chunkable
    file. Used by `just similar-build` for full / incremental rebuilds."""
    paths: list[Path] = []
    for prefix, _kind, _chunker in _DISPATCH:
        target = repo_root / prefix
        if target.is_file():
            paths.append(target)
        elif target.is_dir():
            for p in sorted(target.rglob("*")):
                if not p.is_file():
                    continue
                # Filter to expected extensions per kind.
                if prefix.startswith("docs/") and p.suffix != ".md":
                    continue
                if prefix.startswith("src/") and p.suffix != ".rs":
                    continue
                # Skip template / index files in docs/open-work/.
                if p.name.startswith("_template") or p.name == "open-work.md":
                    continue
                paths.append(p)
    return paths
