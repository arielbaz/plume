"""Prétraitement : audio (yt-dlp, ffmpeg/pydub) et images (binarisation, déskew)."""
from __future__ import annotations

import shutil
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageOps

try:  # OpenCV est recommandé mais pas obligatoire
    import cv2
except ImportError:  # pragma: no cover
    cv2 = None

# --------------------------------------------------------------------- audio

def download_audio(url: str, out_dir: str | Path) -> Path:
    """Télécharge l'audio d'une URL (YouTube, etc.) avec yt-dlp."""
    try:
        import yt_dlp
    except ImportError as e:
        raise RuntimeError("yt-dlp n'est pas installé : pip install yt-dlp") from e
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    opts = {
        "format": "bestaudio/best",
        "outtmpl": str(out_dir / "%(id)s.%(ext)s"),
        "quiet": True,
        "noplaylist": True,
    }
    with yt_dlp.YoutubeDL(opts) as ydl:
        info = ydl.extract_info(url, download=True)
        return Path(ydl.prepare_filename(info))


def to_wav16k(src: str | Path, dst: str | Path | None = None, normalize: bool = True) -> Path:
    """Convertit n'importe quel audio/vidéo en WAV 16 kHz mono (format natif de Whisper).

    Utilise ffmpeg directement ; `loudnorm` homogénéise le volume (utile pour les cours
    enregistrés au téléphone). 
    """
    if shutil.which("ffmpeg") is None:
        raise RuntimeError("ffmpeg est introuvable : installez-le et ajoutez-le au PATH.")
    src = Path(src)
    dst = Path(dst) if dst else Path(tempfile.mkdtemp(prefix="ts_")) / f"{src.stem}.wav"
    cmd = ["ffmpeg", "-y", "-loglevel", "error", "-i", str(src), "-vn", "-ac", "1", "-ar", "16000"]
    if normalize:
        cmd += ["-af", "highpass=f=80,loudnorm=I=-18:TP=-1.5:LRA=11"]
    cmd.append(str(dst))
    subprocess.run(cmd, check=True)
    return dst


def audio_duration(path: str | Path) -> float:
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
        capture_output=True, text=True, check=True,
    )
    return float(out.stdout.strip() or 0)

# --------------------------------------------------------------------- image

def load_gray(path: str | Path) -> np.ndarray:
    img = ImageOps.exif_transpose(Image.open(path)).convert("L")
    return np.asarray(img, dtype=np.uint8)


def _otsu(gray: np.ndarray) -> int:
    hist = np.bincount(gray.ravel(), minlength=256).astype(float)
    total = gray.size
    sum_all = np.dot(np.arange(256), hist)
    w_b = sum_b = 0.0
    best, thr = -1.0, 127
    for t in range(256):
        w_b += hist[t]
        if w_b == 0:
            continue
        w_f = total - w_b
        if w_f == 0:
            break
        sum_b += t * hist[t]
        m_b, m_f = sum_b / w_b, (sum_all - sum_b) / w_f
        between = w_b * w_f * (m_b - m_f) ** 2
        if between > best:
            best, thr = between, t
    return thr


def _local_threshold(gray: np.ndarray, block: int, c: int) -> np.ndarray:
    """Seuil adaptatif (moyenne locale) sans OpenCV, via image intégrale."""
    pad = block // 2
    padded = np.pad(gray.astype(np.float64), pad, mode="edge")
    integral = padded.cumsum(0).cumsum(1)
    integral = np.pad(integral, ((1, 0), (1, 0)))
    h, w = gray.shape
    s = (integral[block:block + h, block:block + w] - integral[:h, block:block + w]
         - integral[block:block + h, :w] + integral[:h, :w])
    mean = s / (block * block)
    return np.where(gray > mean - c, 255, 0).astype(np.uint8)


def binarize(gray: np.ndarray, method: str = "adaptive", block: int = 31, c: int = 12) -> np.ndarray:
    """Texte noir sur fond blanc. method ∈ {otsu, adaptive}."""
    if method == "otsu":
        if cv2 is not None:
            return cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)[1]
        return np.where(gray > _otsu(gray), 255, 0).astype(np.uint8)
    block = block | 1  # impair
    if cv2 is not None:
        return cv2.adaptiveThreshold(gray, 255, cv2.ADAPTIVE_THRESH_MEAN_C,
                                     cv2.THRESH_BINARY, block, c)
    return _local_threshold(gray, block, c)


def deskew(binary: np.ndarray, max_angle: float = 10.0) -> np.ndarray:
    """Redresse la page par maximisation de la variance des projections horizontales."""
    ink = (binary < 128)
    if ink.sum() < 50:
        return binary
    best_angle, best_score = 0.0, -1.0
    small = Image.fromarray(binary).resize((binary.shape[1] // 4 or 1, binary.shape[0] // 4 or 1))
    for angle in np.arange(-max_angle, max_angle + 0.1, 0.5):
        rot = np.asarray(small.rotate(angle, fillcolor=255)) < 128
        score = rot.sum(axis=1).astype(float).var()
        if score > best_score:
            best_angle, best_score = float(angle), score
    if abs(best_angle) < 0.25:
        return binary
    return np.asarray(Image.fromarray(binary).rotate(best_angle, resample=Image.BICUBIC, fillcolor=255))


def prepare_image(path: str | Path, method: str = "adaptive", scale: float = 1.0,
                  block: int = 31, c: int = 12, do_deskew: bool = True) -> Image.Image:
    gray = load_gray(path)
    if scale != 1.0:
        gray = np.asarray(Image.fromarray(gray).resize(
            (int(gray.shape[1] * scale), int(gray.shape[0] * scale)), Image.LANCZOS))
    out = binarize(gray, method, block, c)
    if do_deskew:
        out = deskew(out)
    return Image.fromarray(out)
