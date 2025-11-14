"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any, Union
from enum import Enum
import json

@dataclass
class GetWorkerRequest:
    connection_id: Optional[str]
    prompt: str


@dataclass
class GetWorkerResponse:
    success: bool
    worker_addr: Optional[str]
    worker_label: Optional[str]
    connection_id: str
    message: Optional[str]


@dataclass
class DirectorErrorNoWorkersAvailable:
    pass

@dataclass
class DirectorErrorInvalidRequest:
    field_0: str

DirectorError = Union[
    DirectorErrorNoWorkersAvailable,
    DirectorErrorInvalidRequest
]

def deserialize_directorerror(data: Any) -> DirectorError:
    """Deserialize MessagePack data to DirectorError variant."""
    if not isinstance(data, dict):
        raise ValueError(f"Expected dict for enum, got {type(data)}")
    
    if len(data) != 1:
        raise ValueError(f"Expected single-key dict for enum, got {len(data)} keys")
    
    variant_name, variant_data = next(iter(data.items()))
    
    if variant_name == 'NoWorkersAvailable':
        return DirectorErrorNoWorkersAvailable()
    if variant_name == 'InvalidRequest':
        if isinstance(variant_data, dict):
            return DirectorErrorInvalidRequest(**variant_data)
        elif isinstance(variant_data, list):
            return DirectorErrorInvalidRequest(*variant_data)
        elif variant_data is None:
            return DirectorErrorInvalidRequest()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    
    raise ValueError(f"Unknown variant: {variant_name}")


def serialize_directorerror(value: DirectorError) -> Dict[str, Any]:
    """Serialize DirectorError variant to MessagePack-compatible dict."""
    if isinstance(value, DirectorErrorNoWorkersAvailable):
        return {'NoWorkersAvailable': None}
    if isinstance(value, DirectorErrorInvalidRequest):
        return {'InvalidRequest': [
            value.field_0,
        ]}
    
    raise ValueError(f"Unknown value type: {type(value)}")


