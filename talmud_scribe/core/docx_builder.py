"""Génération Word (.docx) : paragraphes bidirectionnels, hébreu en RTL, français en LTR."""
from __future__ import annotations

import datetime as dt
import re
from pathlib import Path

from docx import Document
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Pt, Cm, RGBColor

_HEBREW = re.compile(r"[֐-׿]")
_LATIN = re.compile(r"[A-Za-zÀ-ÿ]")

LATIN_FONT = "Calibri"
HEBREW_FONT = "David"     # présent avec Word ; LibreOffice retombe sur une police hébraïque


def hebrew_ratio(text: str) -> float:
    heb, lat = len(_HEBREW.findall(text)), len(_LATIN.findall(text))
    return heb / (heb + lat) if heb + lat else 0.0


def is_rtl(text: str, threshold: float = 0.5) -> bool:
    return hebrew_ratio(text) >= threshold


def _set_run_fonts(run, rtl: bool) -> None:
    rpr = run._r.get_or_add_rPr()
    fonts = rpr.find(qn("w:rFonts"))
    if fonts is None:
        fonts = OxmlElement("w:rFonts")
        rpr.append(fonts)
    for attr in ("w:ascii", "w:hAnsi", "w:cs"):
        fonts.set(qn(attr), LATIN_FONT)
    fonts.set(qn("w:cs"), HEBREW_FONT)         # police pour les scripts complexes (hébreu)
    if rtl:
        rpr.append(OxmlElement("w:rtl"))       # le run est lu de droite à gauche
    # taille "complex script" (sinon l'hébreu garde la taille par défaut)
    size = run.font.size
    if size is not None:
        szcs = OxmlElement("w:szCs")
        szcs.set(qn("w:val"), str(int(size.pt * 2)))
        rpr.append(szcs)


def _set_bidi(paragraph, rtl: bool) -> None:
    ppr = paragraph._p.get_or_add_pPr()
    if rtl:
        ppr.append(OxmlElement("w:bidi"))
        paragraph.alignment = WD_ALIGN_PARAGRAPH.JUSTIFY   # en bidi, « left » = début = droite


def _add_page_number(section) -> None:
    p = section.footer.paragraphs[0]
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = p.add_run()
    for kind, text in (("begin", None), (None, "PAGE"), ("end", None)):
        if kind:
            el = OxmlElement("w:fldChar")
            el.set(qn("w:fldCharType"), kind)
        else:
            el = OxmlElement("w:instrText")
            el.set(qn("xml:space"), "preserve")
            el.text = text
        run._r.append(el)


class DocxBuilder:
    def __init__(self, title: str, subtitle: str = "", author: str = "Talmud Scribe"):
        self.doc = Document()
        self.doc.core_properties.title = title
        self.doc.core_properties.author = author
        sec = self.doc.sections[0]
        sec.page_width, sec.page_height = Cm(21), Cm(29.7)
        sec.left_margin = sec.right_margin = Cm(2.2)
        sec.top_margin = sec.bottom_margin = Cm(2.2)
        normal = self.doc.styles["Normal"]
        normal.font.name, normal.font.size = LATIN_FONT, Pt(12)
        normal.paragraph_format.space_after = Pt(8)
        normal.paragraph_format.line_spacing = 1.25
        _add_page_number(sec)

        h = self.doc.add_paragraph()
        h.alignment = WD_ALIGN_PARAGRAPH.CENTER
        r = h.add_run(title)
        r.bold, r.font.size, r.font.color.rgb = True, Pt(22), RGBColor(0x1F, 0x3A, 0x5F)
        _set_run_fonts(r, is_rtl(title))
        _set_bidi_only = is_rtl(title)
        if _set_bidi_only:
            _set_bidi(h, True)
            h.alignment = WD_ALIGN_PARAGRAPH.CENTER
        if subtitle:
            s = self.doc.add_paragraph()
            s.alignment = WD_ALIGN_PARAGRAPH.CENTER
            sr = s.add_run(subtitle)
            sr.italic, sr.font.size, sr.font.color.rgb = True, Pt(11), RGBColor(0x66, 0x66, 0x66)
            _set_run_fonts(sr, False)

    def add_metadata_table(self, rows: dict[str, str]) -> None:
        table = self.doc.add_table(rows=0, cols=2)
        table.style = "Light Grid Accent 1"
        for k, v in rows.items():
            cells = table.add_row().cells
            cells[0].text, cells[1].text = k, str(v)
            cells[0].paragraphs[0].runs[0].bold = True
        self.doc.add_paragraph()

    def add_heading(self, text: str, level: int = 1) -> None:
        p = self.doc.add_paragraph()
        r = p.add_run(text)
        r.bold = True
        r.font.size = Pt({1: 16, 2: 14}.get(level, 12))
        r.font.color.rgb = RGBColor(0x1F, 0x3A, 0x5F)
        rtl = is_rtl(text)
        _set_run_fonts(r, rtl)
        _set_bidi(p, rtl)
        p.paragraph_format.keep_with_next = True

    def add_paragraph(self, text: str) -> None:
        """Ajoute un paragraphe ; direction choisie sur la proportion de lettres hébraïques.

        Un paragraphe mixte est découpé en runs hébreu (RTL) / non hébreu (LTR) pour que
        les citations en hébreu au milieu d'une phrase française s'affichent correctement.
        """
        text = text.strip()
        if not text:
            return
        rtl_par = is_rtl(text)
        p = self.doc.add_paragraph()
        _set_bidi(p, rtl_par)
        if not rtl_par:
            p.alignment = WD_ALIGN_PARAGRAPH.JUSTIFY
        # découpe en blocs consécutifs de même script (les espaces suivent le bloc courant)
        for chunk, heb in _split_scripts(text):
            run = p.add_run(chunk)
            run.font.size = Pt(12)
            _set_run_fonts(run, heb)

    def add_paragraphs(self, paragraphs: list[str]) -> None:
        for t in paragraphs:
            self.add_paragraph(t)

    def save(self, path: str | Path) -> Path:
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        self.doc.save(path)
        return path


_TOKEN = re.compile(r"\S+|\s+")


def _split_scripts(text: str) -> list[tuple[str, bool]]:
    """Regroupe les mots en blocs (texte, est_hébreu). Mots neutres (chiffres, ponctuation)
    et espaces héritent du bloc précédent pour éviter un découpage en miettes."""
    chunks: list[tuple[str, bool]] = []
    current = False
    for tok in _TOKEN.findall(text):
        if tok.isspace():
            heb = current
        elif _HEBREW.search(tok):
            heb = True
        elif _LATIN.search(tok):
            heb = False
        else:
            heb = current
        current = heb
        if chunks and chunks[-1][1] == heb:
            chunks[-1] = (chunks[-1][0] + tok, heb)
        else:
            chunks.append((tok, heb))
    return chunks


def build_document(title: str, paragraphs: list[str], out_path: str | Path,
                   meta: dict[str, str] | None = None) -> Path:
    b = DocxBuilder(title, subtitle=dt.date.today().strftime("%d/%m/%Y"))
    if meta:
        b.add_metadata_table(meta)
    b.add_paragraphs(paragraphs)
    return b.save(out_path)
