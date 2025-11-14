"""Generated inference service"""
from .types import *
from .client import InferenceClient
from .server import InferenceServer, InferenceHandler

__all__ = ['InferenceClient', 'InferenceServer', 'InferenceHandler']
