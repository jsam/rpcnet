"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any
from enum import Enum
import json

@dataclass
class GetWorkerRequest:
    connection_id: Optional[str]
    prompt: str


class DirectorError(Enum):
    NOWORKERSAVAILABLE = 0
    INVALIDREQUEST = 1


@dataclass
class GetWorkerResponse:
    success: bool
    worker_addr: Optional[str]
    worker_label: Optional[str]
    connection_id: str
    message: Optional[str]


