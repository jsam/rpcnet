"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any, Union
from enum import Enum
import json

"""Errors that can occur during computation"""
@dataclass
class ComputeErrorWorkerBusy:
    pass

@dataclass
class ComputeErrorInvalidInput:
    field_0: str

@dataclass
class ComputeErrorProcessingFailed:
    field_0: str

ComputeError = Union[
    ComputeErrorWorkerBusy,
    ComputeErrorInvalidInput,
    ComputeErrorProcessingFailed
]

def deserialize_computeerror(data: Any) -> ComputeError:
    """Deserialize MessagePack data to ComputeError variant."""
    if not isinstance(data, dict):
        raise ValueError(f"Expected dict for enum, got {type(data)}")
    
    if len(data) != 1:
        raise ValueError(f"Expected single-key dict for enum, got {len(data)} keys")
    
    variant_name, variant_data = next(iter(data.items()))
    
    if variant_name == 'WorkerBusy':
        return ComputeErrorWorkerBusy()
    if variant_name == 'InvalidInput':
        if isinstance(variant_data, dict):
            return ComputeErrorInvalidInput(**variant_data)
        elif isinstance(variant_data, list):
            return ComputeErrorInvalidInput(*variant_data)
        elif variant_data is None:
            return ComputeErrorInvalidInput()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    if variant_name == 'ProcessingFailed':
        if isinstance(variant_data, dict):
            return ComputeErrorProcessingFailed(**variant_data)
        elif isinstance(variant_data, list):
            return ComputeErrorProcessingFailed(*variant_data)
        elif variant_data is None:
            return ComputeErrorProcessingFailed()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    
    raise ValueError(f"Unknown variant: {variant_name}")


def serialize_computeerror(value: ComputeError) -> Dict[str, Any]:
    """Serialize ComputeError variant to MessagePack-compatible dict."""
    if isinstance(value, ComputeErrorWorkerBusy):
        return {'WorkerBusy': None}
    if isinstance(value, ComputeErrorInvalidInput):
        return {'InvalidInput': [
            value.field_0,
        ]}
    if isinstance(value, ComputeErrorProcessingFailed):
        return {'ProcessingFailed': [
            value.field_0,
        ]}
    
    raise ValueError(f"Unknown value type: {type(value)}")


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


