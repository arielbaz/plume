import sys
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.asr_engine import Segment, build_prompt, segments_to_paragraphs
from core.docx_builder import _split_scripts, build_document, is_rtl
from utils.cache import Cache, make_key
from utils.metrics import cer, wer
from utils.preprocessor import binarize, prepare_image


def test_rtl_detection():
    assert is_rtl("רש״י אומר שזה נכון")
    assert not is_rtl("Rachi explique que")
    assert not is_rtl("")


def test_split_scripts_mixed():
    chunks = _split_scripts("Rachi dit רש״י אומר שכן 42 donc")
    assert [h for _, h in chunks] == [False, True, False]
    assert "".join(t for t, _ in chunks) == "Rachi dit רש״י אומר שכן 42 donc"


def test_docx_rtl_xml(tmp_path):
    from docx import Document
    out = build_document("Titre", ["Bonjour le monde.", "שלום עולם גדול"], tmp_path / "a.docx")
    doc = Document(out)
    bidi = [p for p in doc.paragraphs if p._p.pPr is not None and p._p.pPr.find(
        "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}bidi") is not None]
    assert len(bidi) == 1 and "שלום" in bidi[0].text


def test_metrics_nikud_and_gershayim():
    assert wer("רש״י אומר", "רש\"י אומר") == 0
    assert cer("שָׁלוֹם", "שלום") == 0
    assert wer("a b c d", "a b x d") == 0.25


def test_paragraph_grouping():
    segs = [Segment(0, 2, "a"), Segment(2.1, 4, "b"), Segment(9, 10, "c")]
    assert segments_to_paragraphs(segs) == ["a b", "c"]


def test_prompt_bounded():
    assert len(build_prompt("x" * 5000)) <= 700


def test_cache_roundtrip(tmp_path):
    f = tmp_path / "x.bin"
    f.write_bytes(b"abc")
    c = Cache(tmp_path / "c")
    k = make_key(f, model="small")
    assert c.get(k) is None
    c.set(k, [{"a": "é"}])
    assert c.get(k) == [{"a": "é"}]
    assert make_key(f, model="medium") != k


@pytest.mark.parametrize("method", ["otsu", "adaptive"])
def test_binarize_separates_ink(method):
    g = np.full((120, 120), 220, np.uint8)
    g[40:80, 40:44] = 20      # trait sombre
    b = binarize(g, method)
    assert b[60, 42] == 0 and b[5, 5] == 255


def test_prepare_image(tmp_path):
    p = tmp_path / "i.png"
    Image.fromarray(np.full((100, 100), 200, np.uint8)).save(p)
    assert prepare_image(p, scale=2.0).size == (200, 200)
