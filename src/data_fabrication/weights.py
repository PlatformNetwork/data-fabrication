"""Platform weight computation."""

from __future__ import annotations

from .repository import DataFabricationRepository


async def get_weights(repository: DataFabricationRepository) -> dict[str, float]:
    """Return best completed score per miner hotkey."""

    return await repository.weights()
