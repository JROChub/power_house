"""Power House Archive and Rootprint v1 core SDK.

External proof attachments are intentionally absent from this namespace.
Import ``power_house.external`` explicitly when attachment transport is needed.
"""

from .core import (
    PHA_SCHEMA_V1,
    ROOTPRINT_SCHEMA_V1,
    PowerHouseError,
    calculate_phx_fingerprint,
    create_artifact,
    equivalent,
    fork,
    merge,
    navigate,
    new_rootprint,
    verify_artifact,
    verify_rootprint,
)

__all__ = [
    "PHA_SCHEMA_V1",
    "ROOTPRINT_SCHEMA_V1",
    "PowerHouseError",
    "calculate_phx_fingerprint",
    "create_artifact",
    "equivalent",
    "fork",
    "merge",
    "navigate",
    "new_rootprint",
    "verify_artifact",
    "verify_rootprint",
]

__version__ = "0.3.5"
