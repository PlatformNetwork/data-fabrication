"""ZIP artifact validation and extraction for miner harness submissions."""

from __future__ import annotations

import hashlib
import json
import stat
import zipfile
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any

import yaml

ALLOWED_EXTENSIONS = {
    ".py",
    ".txt",
    ".md",
    ".yaml",
    ".yml",
    ".json",
    ".jsonl",
    ".toml",
    ".cfg",
    ".ini",
}


class ArtifactError(ValueError):
    """Raised when a submitted ZIP artifact is invalid."""


@dataclass(frozen=True)
class ArtifactFile:
    path: str
    size: int
    sha256: str


@dataclass(frozen=True)
class SubmissionArtifact:
    submission_id: str
    source_zip_path: Path
    workspace_path: Path
    entrypoint: str
    artifact_hash: str
    files: list[ArtifactFile]
    manifest: dict[str, Any] = field(default_factory=dict)

    def to_json(self) -> str:
        return json.dumps(
            {
                "source_zip_path": str(self.source_zip_path),
                "workspace_path": str(self.workspace_path),
                "entrypoint": self.entrypoint,
                "artifact_hash": self.artifact_hash,
                "files": [file.__dict__ for file in self.files],
                "manifest": self.manifest,
            },
            separators=(",", ":"),
        )


def validate_and_extract_zip(
    *,
    zip_bytes: bytes,
    submission_id: str,
    artifact_root: Path,
    max_size_bytes: int,
    max_files: int,
    max_uncompressed_bytes: int,
) -> SubmissionArtifact:
    """Validate a miner ZIP and extract it into an isolated artifact workspace."""

    if not zip_bytes:
        raise ArtifactError("empty ZIP artifact")
    if len(zip_bytes) > max_size_bytes:
        raise ArtifactError("ZIP artifact exceeds maximum upload size")

    artifact_hash = hashlib.sha256(zip_bytes).hexdigest()
    root = artifact_root / submission_id
    source_zip_path = root / "source.zip"
    workspace_path = root / "workspace"
    root.mkdir(parents=True, exist_ok=True)
    workspace_path.mkdir(parents=True, exist_ok=True)
    source_zip_path.write_bytes(zip_bytes)

    files: list[ArtifactFile] = []
    total_uncompressed = 0
    try:
        archive_context = zipfile.ZipFile(source_zip_path)
    except zipfile.BadZipFile as exc:
        raise ArtifactError("submission must be a valid ZIP archive") from exc

    with archive_context as archive:
        infos = [info for info in archive.infolist() if not info.is_dir()]
        if not infos:
            raise ArtifactError("ZIP does not contain files")
        if len(infos) > max_files:
            raise ArtifactError("ZIP contains too many files")
        for info in infos:
            safe_path = _safe_member_path(info)
            total_uncompressed += info.file_size
            if total_uncompressed > max_uncompressed_bytes:
                raise ArtifactError("ZIP uncompressed size is too large")
            target = workspace_path / safe_path
            target.parent.mkdir(parents=True, exist_ok=True)
            data = archive.read(info)
            target.write_bytes(data)
            files.append(
                ArtifactFile(
                    path=safe_path,
                    size=len(data),
                    sha256=hashlib.sha256(data).hexdigest(),
                )
            )

    manifest = _load_manifest(workspace_path)
    entrypoint = _safe_entrypoint(str(manifest.get("entrypoint") or "harness.py"))
    if not (workspace_path / entrypoint).is_file():
        py_files = sorted(file.path for file in files if file.path.endswith(".py"))
        if len(py_files) == 1:
            entrypoint = py_files[0]
        else:
            raise ArtifactError("ZIP must contain harness.py or manifest entrypoint")

    return SubmissionArtifact(
        submission_id=submission_id,
        source_zip_path=source_zip_path,
        workspace_path=workspace_path,
        entrypoint=entrypoint,
        artifact_hash=artifact_hash,
        files=files,
        manifest=manifest,
    )


def _safe_member_path(info: zipfile.ZipInfo) -> str:
    name = info.filename.replace("\\", "/")
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise ArtifactError(f"unsafe ZIP path: {info.filename}")
    if not path.name:
        raise ArtifactError(f"unsafe ZIP path: {info.filename}")
    mode = info.external_attr >> 16
    if stat.S_ISLNK(mode):
        raise ArtifactError(f"symlinks are not allowed: {info.filename}")
    if path.suffix and path.suffix.lower() not in ALLOWED_EXTENSIONS:
        raise ArtifactError(f"file extension is not allowed: {info.filename}")
    return path.as_posix()


def _load_manifest(workspace_path: Path) -> dict[str, Any]:
    for name in ("data_fabrication.yaml", "data_fabrication.yml", "manifest.yaml"):
        path = workspace_path / name
        if path.is_file():
            data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
            if not isinstance(data, dict):
                raise ArtifactError("manifest must be a mapping")
            return data
    return {}


def _safe_entrypoint(value: str) -> str:
    path = PurePosixPath(value.replace("\\", "/"))
    if path.is_absolute() or ".." in path.parts or path.suffix != ".py":
        raise ArtifactError("manifest entrypoint must be a relative Python file")
    return path.as_posix()
