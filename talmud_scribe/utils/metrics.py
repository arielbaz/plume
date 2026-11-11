"""WER / CER sans dépendance, avec normalisation adaptée à l'hébreu."""
from __future__ import annotations

import re
import unicodedata

_NIKUD = re.compile(r"[֑-ׇ]")          # cantillation + nikud
_PUNCT = re.compile(r"[^\w\sא-ת]", re.UNICODE)
_GERSHAYIM = str.maketrans({"״": '"', "׳": "'", "’": "'", "“": '"', "”": '"'})


def normalize(text: str, keep_punct: bool = False) -> str:
    text = unicodedata.normalize("NFC", text).translate(_GERSHAYIM)
    text = _NIKUD.sub("", text).lower()
    if not keep_punct:
        text = _PUNCT.sub(" ", text)
    return re.sub(r"\s+", " ", text).strip()


def _levenshtein(a: list, b: list) -> int:
    if not a:
        return len(b)
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def wer(reference: str, hypothesis: str) -> float:
    ref, hyp = normalize(reference).split(), normalize(hypothesis).split()
    return _levenshtein(ref, hyp) / max(len(ref), 1)


def cer(reference: str, hypothesis: str) -> float:
    ref, hyp = list(normalize(reference).replace(" ", "")), list(normalize(hypothesis).replace(" ", ""))
    return _levenshtein(ref, hyp) / max(len(ref), 1)
