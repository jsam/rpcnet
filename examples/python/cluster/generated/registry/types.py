"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any, Union
from enum import Enum
import json

"""Response with worker information"""
@dataclass
class GetWorkerResponse:
    worker_addr: str
    worker_id: str


"""Errors from registry operations"""
@dataclass
class RegistryErrorNoWorkersAvailable:
    pass

@dataclass
class RegistryErrorInvalidRequest:
    field_0: str

RegistryError = Union[
    RegistryErrorNoWorkersAvailable,
    RegistryErrorInvalidRequest
]

def deserialize_registryerror(data: Any) -> RegistryError:
    """Deserialize MessagePack data to RegistryError variant."""
    if not isinstance(data, dict):
        raise ValueError(f"Expected dict for enum, got {type(data)}")
    
    if len(data) != 1:
        raise ValueError(f"Expected single-key dict for enum, got {len(data)} keys")
    
    variant_name, variant_data = next(iter(data.items()))
    
    if variant_name == 'NoWorkersAvailable':
        return RegistryErrorNoWorkersAvailable()
    if variant_name == 'InvalidRequest':
        if isinstance(variant_data, dict):
            return RegistryErrorInvalidRequest(**variant_data)
        elif isinstance(variant_data, list):
            return RegistryErrorInvalidRequest(*variant_data)
        elif variant_data is None:
            return RegistryErrorInvalidRequest()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    
    raise ValueError(f"Unknown variant: {variant_name}")


def serialize_registryerror(value: RegistryError) -> Dict[str, Any]:
    """Serialize RegistryError variant to MessagePack-compatible dict."""
    if isinstance(value, RegistryErrorNoWorkersAvailable):
        return {'NoWorkersAvailable': None}
    if isinstance(value, RegistryErrorInvalidRequest):
        return {'InvalidRequest': [
            value.field_0,
        ]}
    
    raise ValueError(f"Unknown value type: {type(value)}")


"""Request to get an available worker"""
@dataclass
class GetWorkerRequest:
    client_id: str


