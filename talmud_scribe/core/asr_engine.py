"""Transcription Whisper (faster-whisper, int8 CPU) avec amorçage par le patron vocal."""
from __future__ import annotations

import json
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Callable, Iterable

ROOT = Path(__file__).resolve().parent.parent
CALIB = ROOT / "calibration"
PROFILE = CALIB / "profile.json"

# Termes systématiquement fournis au modèle (jargon talmudique translittéré).
BASE_HOTWORDS = (
    "Guemara, Mishna, Tana, Tanaïm, Amora, Amoraïm, Kashya, Teyku, Mai shna, Ta chema, "
    "Nafka mina, Machloket, Halakha, Rachi, Tosfot, Rambam, Rav, Rava, Abayé, Hadran alakh"
)


# Presets : alias -> identifiant faster-whisper / Hugging Face (format CTranslate2).
# Le modèle ivrit.ai est affiné sur l'hébreu : à tester en priorité sur le patron vocal.
# Sur un i5 / 8 Go, large-v3-turbo est lent (~1-2x la durée audio) : préférez "small"/"medium" pour itérer.
MODEL_PRESETS = {
    "tiny": "tiny", "base": "base", "small": "small", "medium": "medium",
    "large-v3-turbo": "large-v3-turbo",
    "ivrit-turbo": "ivrit-ai/whisper-large-v3-turbo-ct2",
    "ivrit-large": "ivrit-ai/whisper-large-v3-ct2",
}


@dataclass
class Segment:
    start: float
    end: float
    text: str
    avg_logprob: float = 0.0

    def to_dict(self) -> dict:
        return asdict(self)


def load_profile() -> dict:
    try:
        return json.loads(PROFILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def build_prompt(vocal_text: str | None = None, extra: str = "") -> str:
    """Prompt contextuel : Whisper imite le style/vocabulaire du texte précédent.

    On le limite (~224 tokens max côté modèle) en gardant la fin du patron, plus pertinente.
    """
    if vocal_text is None:
        p = CALIB / "vocal_text.txt"
        vocal_text = p.read_text(encoding="utf-8") if p.exists() else ""
    parts = [extra.strip(), vocal_text.strip()]
    prompt = " ".join(x for x in parts if x)
    return prompt[-700:]


class ASREngine:
    def __init__(self, model_size: str = "small", device: str = "cpu",
                 compute_type: str = "int8", cpu_threads: int = 0,
                 model_dir: str | None = None):
        try:
            from faster_whisper import WhisperModel
        except ImportError as e:
            raise RuntimeError("faster-whisper n'est pas installé : pip install faster-whisper") from e
        self.model_size = MODEL_PRESETS.get(model_size, model_size)
        self.model = WhisperModel(self.model_size, device=device, compute_type=compute_type,
                                  cpu_threads=cpu_threads, download_root=model_dir)

    def transcribe(self, wav_path: str | Path, language: str | None = "fr",
                   initial_prompt: str | None = None, beam_size: int | None = None,
                   progress: Callable[[float], None] | None = None) -> list[Segment]:
        """language="fr" par défaut : les cours mêlent hébreu/araméen mais la langue
        dominante est le français ; None laisse Whisper détecter (moins stable en mélange)."""
        prof = load_profile().get("asr", {})
        beam = beam_size or prof.get("beam_size", 5)
        segments, info = self.model.transcribe(
            str(wav_path),
            language=language,
            task="transcribe",
            beam_size=beam,
            initial_prompt=initial_prompt if initial_prompt is not None else build_prompt(),
            hotwords=BASE_HOTWORDS,
            vad_filter=True,
            vad_parameters={"min_silence_duration_ms": 500},
            condition_on_previous_text=prof.get("condition_on_previous_text", False),
            temperature=[0.0, 0.2, 0.4],
            no_speech_threshold=0.6,
        )
        out: list[Segment] = []
        total = max(info.duration, 1e-6)
        for s in segments:  # générateur : le décodage a lieu ici
            out.append(Segment(s.start, s.end, s.text.strip(), s.avg_logprob))
            if progress:
                progress(min(s.end / total, 1.0))
        return out


def segments_to_paragraphs(segments: Iterable[Segment], gap: float = 1.5, max_chars: int = 700) -> list[str]:
    """Regroupe les segments en paragraphes (pause longue ou paragraphe trop long)."""
    paras: list[str] = []
    cur: list[str] = []
    last_end = None
    for s in segments:
        if not s.text:
            continue
        if cur and ((last_end is not None and s.start - last_end > gap) or sum(map(len, cur)) > max_chars):
            paras.append(" ".join(cur))
            cur = []
        cur.append(s.text)
        last_end = s.end
    if cur:
        paras.append(" ".join(cur))
    return paras


def calibrate(wav_path: str | Path | None = None, truth_txt: str | Path | None = None,
              models: list[str] | None = None, language: str | None = "fr",
              verbose: bool = True) -> dict:
    """Compare des modèles / avec-sans prompt sur le patron vocal (WER) et mémorise le meilleur."""
    from utils.metrics import wer
    from utils.preprocessor import to_wav16k

    wav_path = Path(wav_path or CALIB / "vocal_patron.wav")
    truth_txt = Path(truth_txt or CALIB / "vocal_text.txt")
    if not wav_path.exists():
        raise FileNotFoundError(f"{wav_path} manquant — voir calibration/README.md")
    truth = truth_txt.read_text(encoding="utf-8")
    wav = to_wav16k(wav_path)
    results = []
    for name in models or ["small", "ivrit-turbo"]:
        eng = ASREngine(name)
        for use_prompt in (False, True):
            # Le patron est à la fois prompt et vérité terrain : le score "avec prompt" est
            # donc optimiste. On ne prompt qu'avec la liste de termes (hotwords) pour rester honnête.
            prompt = BASE_HOTWORDS if use_prompt else ""
            text = " ".join(s.text for s in eng.transcribe(wav, language, initial_prompt=prompt))
            score = wer(truth, text)
            results.append({"model": name, "prompt": use_prompt, "wer": round(score, 4)})
            if verbose:
                print(f"  WER {score:6.1%}  modèle={name} prompt={use_prompt}")
        del eng
    best = min(results, key=lambda r: r["wer"])
    try:
        profile = json.loads(PROFILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        profile = {}
    profile["asr"] = {**profile.get("asr", {}), "model": best["model"], "wer": best["wer"]}
    PROFILE.write_text(json.dumps(profile, ensure_ascii=False, indent=2), encoding="utf-8")
    return profile["asr"]


def check_patron(engine: "ASREngine", patron_wav: str | Path, truth_text: str,
                 language: str | None = "fr") -> float:
    """WER de ce modèle sur un patron vocal fourni pour CE lancement (aucune sauvegarde).
    Mesuré sans prompt de patron, pour ne pas fausser le score."""
    from utils.preprocessor import to_wav16k
    from utils.metrics import wer
    segs = engine.transcribe(to_wav16k(patron_wav), language, initial_prompt=BASE_HOTWORDS)
    return wer(truth_text, " ".join(s.text for s in segs))
