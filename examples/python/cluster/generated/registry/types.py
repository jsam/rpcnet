"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any
from enum import Enum
import json

"""Response with worker information"""
@dataclass
class GetWorkerResponse:
    worker_addr: str
    worker_id: str


"""Errors from registry operations"""
class RegistryError(Enum):
    NOWORKERSAVAILABLE = 0
    INVALIDREQUEST = 1


"""Request to get an available worker"""
@dataclass
class GetWorkerRequest:
    client_id: str


