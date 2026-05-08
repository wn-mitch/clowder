"""Embedding backend for `just similar`.

Local-only: `fastembed` with `BAAI/bge-small-en-v1.5`. 384-dim,
L2-normalized output (so cosine similarity reduces to a dot product).

Why fastembed: ONNX runtime, no torch dependency, deterministic, and
the model + runtime fit in ~85MB cached. The first call downloads the
model to `~/.cache/fastembed/`; subsequent calls are instant.

Why BGE-small-en-v1.5: top-tier on MTEB retrieval at small size, 384
dims keeps the index file ~5MB at 2.7k chunks, and full-corpus
embedding completes in ~100s on commodity hardware.

We tested `jinaai/jina-embeddings-v2-base-code` (768-dim, code-aware,
30 programming languages) on 2026-05-08 to see if its joint
prose+code training improved retrieval on bare technical identifiers
like `cat_presence`. It did not — quality went *down* on every query
in the comparison set:

  - `just similar 189` lost ticket 194 (the meta-analysis of 189).
  - `just similar 'cat_presence'` lost ticket 120 ("shadow-fox spawn
    vs cat-presence coupling" — has the term in the title!) and 101
    (the influence-map cluster ticket).
  - `just similar 'predator exposure patrol'` --corpus dses lost
    `hide` and `hunt` from top-5; replaced with `apply_remedy_target`
    and `practice_magic` (unrelated to the query).

Lesson: pick embedders by training objective, not training corpus.
BGE-small-v1.5 is retrieval-tuned with hard-negative contrastive
loss; jina-base-code is a generalist code-search model. On a corpus
that's 95% English prose with sprinkled identifiers, the retrieval-
specific smaller model dominates. The swap remains a one-line change
if a future model lands that's both retrieval-tuned AND code-aware.
"""

from __future__ import annotations

from typing import Sequence

import numpy as np


EMBEDDER_NAME = "fastembed:bge-small-en-v1.5"
_MODEL_NAME = "BAAI/bge-small-en-v1.5"
_EMBEDDING_DIM = 384

_model = None  # lazy-init on first use; loading takes ~2s.


def _get_model():
    """Lazy-init the fastembed model. Defer the import so unrelated
    code paths (chunkers tests, --help) don't pay the import cost."""
    global _model
    if _model is None:
        from fastembed import TextEmbedding  # type: ignore[import-not-found]
        _model = TextEmbedding(model_name=_MODEL_NAME)
    return _model


# Embedding batch size — keeps the ONNX session's working set bounded
# regardless of how many chunks the caller hands over. 16 is a
# conservative number that holds for jina-base-code (768-dim, ~160M
# params) on commodity hardware without OOMing on macOS Metal during
# the first-call warm-up window.
_BATCH_SIZE = 16


def embed_batch(texts: Sequence[str], *, progress: bool = False) -> np.ndarray:
    """Embed a batch of texts. Returns an `(N, EMBEDDING_DIM)` float32
    array of L2-normalized vectors so cosine similarity is just a dot
    product. Empty input returns an empty `(0, EMBEDDING_DIM)` array
    so callers can `np.vstack` without a special case.

    Internally chunks the input into `_BATCH_SIZE` mini-batches so
    fastembed's ONNX session has bounded working set, even when the
    caller passes thousands of chunks at once."""
    if not texts:
        return np.zeros((0, _EMBEDDING_DIM), dtype=np.float32)
    model = _get_model()
    out: list[np.ndarray] = []
    texts_list = list(texts)
    import sys
    n = len(texts_list)
    for start in range(0, n, _BATCH_SIZE):
        batch = texts_list[start : start + _BATCH_SIZE]
        vectors = list(model.embed(batch))
        out.append(np.asarray(vectors, dtype=np.float32))
        if progress:
            done = min(start + _BATCH_SIZE, n)
            print(f"  embedded {done}/{n} chunks", file=sys.stderr, flush=True)
    arr = np.vstack(out) if len(out) > 1 else out[0]
    # Normalize defensively — both BGE and Jina-code return normalized
    # outputs by default, but the rest of the pipeline assumes unit-
    # norm and we'd rather pay a cheap re-normalize than audit upstream.
    norms = np.linalg.norm(arr, axis=1, keepdims=True)
    norms[norms == 0] = 1.0
    return arr / norms


def embedding_dim() -> int:
    return _EMBEDDING_DIM
