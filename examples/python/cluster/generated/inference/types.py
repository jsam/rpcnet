"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any
from enum import Enum
import json

class InferenceResponse(Enum):
    CONNECTED = 0
    TOKEN = 1
    ERROR = 2
    DONE = 3


class InferenceError(Enum):
    WORKERFAILED = 0
    INVALIDREQUEST = 1


@dataclass
class InferenceRequest:
    connection_id: str
    prompt: str


