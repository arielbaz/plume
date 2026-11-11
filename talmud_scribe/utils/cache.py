"""Cache disque : évite de retraiter un fichier déjà transcrit avec les mêmes réglages."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

DEFAULT_DIR = Path.home() / ".cache" / "talmud_scribe"


def file_hash(path: str | Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while block := f.read(chunk):
            h.update(block)
    return h.hexdigest()


def make_key(path: str | Path, **params: Any) -> str:
    """Clé = hash du contenu + réglages (modèle, langue, prompt...)."""
    blob = json.dumps(params, sort_keys=True, ensure_ascii=False, default=str)
    return hashlib.sha256((file_hash(path) + blob).encode()).hexdigest()[:32]


class Cache:
    def __init__(self, directory: str | Path | None = None, enabled: bool = True):
        self.dir = Path(directory) if directory else DEFAULT_DIR
        self.enabled = enabled

    def _path(self, key: str) -> Path:
        return self.dir / f"{key}.json"

    def get(self, key: str) -> Any | None:
        if not self.enabled:
            return None
        p = self._path(key)
        try:
            return json.loads(p.read_text(encoding="utf-8")) if p.exists() else None
        except (OSError, json.JSONDecodeError):
            return None

    def set(self, key: str, value: Any) -> None:
        if not self.enabled:
            return
        self.dir.mkdir(parents=True, exist_ok=True)
        tmp = self._path(key).with_suffix(".tmp")
        tmp.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
        tmp.replace(self._path(key))   # écriture atomique
