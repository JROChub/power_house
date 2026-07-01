"""Power House Archive and Rootprint v1 core SDK.

External proof attachments are intentionally absent from this namespace.
Import ``power_house.external`` explicitly when attachment transport is needed.
"""

from .core import (
    PHA_SCHEMA_V1,
    ROOTPRINT_SCHEMA_V1,
    MEMORY_CAPSULE_SCHEMA_V1,
    MemoryVerificationReport,
    PowerHouseError,
    calculate_memory_capsule_digest,
    calculate_memory_core_digest,
    calculate_phx_fingerprint,
    create_artifact,
    create_identity,
    equivalent,
    equivalent_identity,
    equivalent_rootprints,
    fork,
    fork_identity,
    merge,
    merge_identity,
    merge_rootprints,
    navigate,
    new_rootprint,
    load_memory_capsule,
    replay_identity,
    replay_rootprint,
    semantic_packet_digest,
    verify_artifact,
    verify_identity,
    verify_memory_capsule,
    verify_rootprint,
)

__all__ = [
    "PHA_SCHEMA_V1",
    "ROOTPRINT_SCHEMA_V1",
    "MEMORY_CAPSULE_SCHEMA_V1",
    "MemoryVerificationReport",
    "PowerHouseError",
    "calculate_memory_capsule_digest",
    "calculate_memory_core_digest",
    "calculate_phx_fingerprint",
    "create_artifact",
    "create_identity",
    "equivalent",
    "equivalent_identity",
    "equivalent_rootprints",
    "fork",
    "fork_identity",
    "merge",
    "merge_identity",
    "merge_rootprints",
    "navigate",
    "new_rootprint",
    "load_memory_capsule",
    "replay_identity",
    "replay_rootprint",
    "semantic_packet_digest",
    "verify_artifact",
    "verify_identity",
    "verify_memory_capsule",
    "verify_rootprint",
]

__version__ = "0.3.20"
