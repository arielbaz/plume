import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.asr_engine import Segment, hebrew_probability, refine_segments


def test_hebrew_probability_codes():
    assert hebrew_probability([("fr", 0.2), ("he", 0.7)]) == 0.7
    assert hebrew_probability([("iw", 0.6)]) == 0.6
    assert hebrew_probability([("fr", 1.0)]) == 0.0
    assert hebrew_probability(None) == 0.0


def _run(segs, p_he, **kw):
    audio = np.zeros(16000 * 30, dtype=np.float32)
    calls = []

    def detect(chunk):
        calls.append(len(chunk))
        return p_he.pop(0)

    return refine_segments(segs, audio, detect, lambda c: "שלום עולם", **kw), calls


def test_hebrew_segment_replaced_french_kept():
    segs = [Segment(0, 4, "bla bla"), Segment(5, 9, "kachia")]
    out, _ = _run(segs, [0.1, 0.9])
    assert [s.lang for s in out] == ["fr", "he"]
    assert out[0].text == "bla bla" and out[1].text == "שלום עולם"


def test_short_segments_skipped():
    out, calls = _run([Segment(0, 1.0, "oui")], [0.99])
    assert calls == [] and out[0].text == "oui" and out[0].lang == "fr"


def test_empty_hebrew_pass_keeps_original():
    audio = np.zeros(16000 * 10, dtype=np.float32)
    out = refine_segments([Segment(0, 4, "original")], audio, lambda c: 0.9, lambda c: "  ")
    assert out[0].text == "original" and out[0].lang == "fr"


def test_chunk_clamped_to_audio():
    out, calls = _run([Segment(28, 30, "fin")], [0.1])
    assert calls[0] <= 16000 * 3 and out[0].lang == "fr"
