"""Generated type definitions for RPC service"""
from dataclasses import dataclass
from typing import Optional, List, Dict, Any, Union
from enum import Enum
import json

@dataclass
class InferenceErrorWorkerFailed:
    field_0: str

@dataclass
class InferenceErrorInvalidRequest:
    field_0: str

InferenceError = Union[
    InferenceErrorWorkerFailed,
    InferenceErrorInvalidRequest
]

def deserialize_inferenceerror(data: Any) -> InferenceError:
    """Deserialize MessagePack data to InferenceError variant."""
    if not isinstance(data, dict):
        raise ValueError(f"Expected dict for enum, got {type(data)}")
    
    if len(data) != 1:
        raise ValueError(f"Expected single-key dict for enum, got {len(data)} keys")
    
    variant_name, variant_data = next(iter(data.items()))
    
    if variant_name == 'WorkerFailed':
        if isinstance(variant_data, dict):
            return InferenceErrorWorkerFailed(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceErrorWorkerFailed(*variant_data)
        elif variant_data is None:
            return InferenceErrorWorkerFailed()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    if variant_name == 'InvalidRequest':
        if isinstance(variant_data, dict):
            return InferenceErrorInvalidRequest(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceErrorInvalidRequest(*variant_data)
        elif variant_data is None:
            return InferenceErrorInvalidRequest()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    
    raise ValueError(f"Unknown variant: {variant_name}")


def serialize_inferenceerror(value: InferenceError) -> Dict[str, Any]:
    """Serialize InferenceError variant to MessagePack-compatible dict."""
    if isinstance(value, InferenceErrorWorkerFailed):
        return {'WorkerFailed': [
            value.field_0,
        ]}
    if isinstance(value, InferenceErrorInvalidRequest):
        return {'InvalidRequest': [
            value.field_0,
        ]}
    
    raise ValueError(f"Unknown value type: {type(value)}")


@dataclass
class InferenceRequest:
    connection_id: str
    prompt: str


@dataclass
class InferenceResponseConnected:
    worker: str
    connection_id: str

@dataclass
class InferenceResponseToken:
    text: str
    sequence: int

@dataclass
class InferenceResponseError:
    message: str

@dataclass
class InferenceResponseDone:
    pass

InferenceResponse = Union[
    InferenceResponseConnected,
    InferenceResponseToken,
    InferenceResponseError,
    InferenceResponseDone
]

def deserialize_inferenceresponse(data: Any) -> InferenceResponse:
    """Deserialize MessagePack data to InferenceResponse variant."""
    if not isinstance(data, dict):
        raise ValueError(f"Expected dict for enum, got {type(data)}")
    
    if len(data) != 1:
        raise ValueError(f"Expected single-key dict for enum, got {len(data)} keys")
    
    variant_name, variant_data = next(iter(data.items()))
    
    if variant_name == 'Connected':
        if isinstance(variant_data, dict):
            return InferenceResponseConnected(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceResponseConnected(*variant_data)
        elif variant_data is None:
            return InferenceResponseConnected()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    if variant_name == 'Token':
        if isinstance(variant_data, dict):
            return InferenceResponseToken(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceResponseToken(*variant_data)
        elif variant_data is None:
            return InferenceResponseToken()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    if variant_name == 'Error':
        if isinstance(variant_data, dict):
            return InferenceResponseError(**variant_data)
        elif isinstance(variant_data, list):
            return InferenceResponseError(*variant_data)
        elif variant_data is None:
            return InferenceResponseError()
        else:
            raise ValueError(f"Unexpected variant data type: {type(variant_data)}")
    if variant_name == 'Done':
        return InferenceResponseDone()
    
    raise ValueError(f"Unknown variant: {variant_name}")


def serialize_inferenceresponse(value: InferenceResponse) -> Dict[str, Any]:
    """Serialize InferenceResponse variant to MessagePack-compatible dict."""
    if isinstance(value, InferenceResponseConnected):
        return {'Connected': {
            'worker': value.worker,
            'connection_id': value.connection_id,
        }}
    if isinstance(value, InferenceResponseToken):
        return {'Token': {
            'text': value.text,
            'sequence': value.sequence,
        }}
    if isinstance(value, InferenceResponseError):
        return {'Error': {
            'message': value.message,
        }}
    if isinstance(value, InferenceResponseDone):
        return {'Done': None}
    
    raise ValueError(f"Unknown value type: {type(value)}")


