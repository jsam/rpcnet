"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any
from enum import Enum
import json

"""Response from compute task"""
@dataclass
class ComputeResponse:
    task_id: str
    result: str
    worker_id: str


"""Request for compute task"""
@dataclass
class ComputeRequest:
    task_id: str
    data: str


"""Errors that can occur during computation"""
class ComputeError(Enum):
    WORKERBUSY = 0
    INVALIDINPUT = 1
    PROCESSINGFAILED = 2


