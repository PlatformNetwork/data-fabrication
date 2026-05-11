from pathlib import Path

import pytest

from data_fabrication.sdk.executors.docker import (
    DockerExecutor,
    DockerLimits,
    DockerMount,
    DockerRunSpec,
)


def test_build_run_command_includes_platform_labels(tmp_path: Path) -> None:
    source = tmp_path / "workspace"
    source.mkdir()
    executor = DockerExecutor(challenge="data-fabrication", allowed_images=("python:3.12-slim",))
    spec = DockerRunSpec(
        image="python:3.12-slim",
        command=("python", "/workspace/harness.py"),
        mounts=(DockerMount(source, "/workspace"),),
        labels={"platform.job": "abc"},
        limits=DockerLimits(network="none"),
    )
    command = executor.build_run_command(spec, "container-name")
    assert "--cap-drop" in command
    assert "platform.challenge=data-fabrication" in command
    assert "python:3.12-slim" in command


def test_rejects_disallowed_image(tmp_path: Path) -> None:
    source = tmp_path / "workspace"
    source.mkdir()
    executor = DockerExecutor(challenge="data-fabrication", allowed_images=("python:3.12-slim",))
    spec = DockerRunSpec(
        image="ubuntu:latest",
        command=("python", "x.py"),
        mounts=(DockerMount(source, "/workspace"),),
    )
    with pytest.raises(RuntimeError):
        executor.build_run_command(spec, "container-name")
