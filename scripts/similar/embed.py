"""Embedding backend for `just similar`.

Local-only: `fastembed` with `BAAI/bge-small-en-v1.5`. 384-dim,
L2-normalized output (so cosine similarity reduces to a dot product).

Why fastembed: ONNX runtime, no torch dependency, deterministic, and
the model + runtime fit in ~85MB cached. The first call downloads the
model to `~/.cache/fastembed/`; subsequent calls are instant.

Why BGE-small-en-v1.5: top-tier on MTEB retrieval at small size, 384
dims keeps the index file ~5MB at 3.7k chunks. Tradeoff: trained on
natural-language web text — Rust doc-comments mixing prose with
`Has<>` / `Without<>` type signatures may retrieve weaker than pure
prose. If that becomes a real problem, swap the EMBEDDER constant
for a code-aware model; the chunk-id and metadata layout don't care.
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


def embed_batch(texts: Sequence[str]) -> np.ndarray:
    """Embed a batch of texts. Returns an `(N, EMBEDDING_DIM)` float32
    array of L2-normalized vectors so cosine similarity is just a dot
    product. Empty input returns an empty `(0, EMBEDDING_DIM)` array
    so callers can `np.vstack` without a special case."""
    if not texts:
        return np.zeros((0, _EMBEDDING_DIM), dtype=np.float32)
    model = _get_model()
    vectors = list(model.embed(list(texts)))
    arr = np.asarray(vectors, dtype=np.float32)
    # fastembed already returns normalized BGE outputs, but normalize
    # defensively — it's cheap and means the rest of the pipeline can
    # assume unit-norm without auditing the upstream library.
    norms = np.linalg.norm(arr, axis=1, keepdims=True)
    norms[norms == 0] = 1.0
    return arr / norms


def embedding_dim() -> int:
    return _EMBEDDING_DIM
