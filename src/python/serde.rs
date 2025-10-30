//! Serialization bridge between Python and Rust using bincode.
//!
//! This module provides utilities to convert between Python objects and bincode-serialized bytes.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A generic value that can be serialized/deserialized between Python and Rust.
///
/// This acts as an intermediate representation that can be converted to/from
/// Python objects and serialized with bincode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerdeValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    List(Vec<SerdeValue>),
    Dict(HashMap<String, SerdeValue>),
}

impl SerdeValue {
    /// Convert a Python object to a SerdeValue
    pub fn from_python(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        if obj.is_none() {
            Ok(SerdeValue::Null)
        } else if let Ok(val) = obj.extract::<bool>() {
            Ok(SerdeValue::Bool(val))
        } else if let Ok(val) = obj.extract::<i64>() {
            Ok(SerdeValue::I64(val))
        } else if let Ok(val) = obj.extract::<f64>() {
            Ok(SerdeValue::F64(val))
        } else if let Ok(val) = obj.extract::<String>() {
            Ok(SerdeValue::String(val))
        } else if let Ok(list) = obj.downcast::<PyList>() {
            let mut values = Vec::new();
            for item in list.iter() {
                values.push(SerdeValue::from_python(&item)?);
            }
            Ok(SerdeValue::List(values))
        } else if let Ok(dict) = obj.downcast::<PyDict>() {
            let mut map = HashMap::new();
            for (key, value) in dict.iter() {
                let key_str = key.extract::<String>()?;
                map.insert(key_str, SerdeValue::from_python(&value)?);
            }
            Ok(SerdeValue::Dict(map))
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                format!("Cannot convert Python type {} to SerdeValue", obj.get_type().name()?),
            ))
        }
    }

    /// Convert a SerdeValue to a Python object
    pub fn to_python<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            SerdeValue::Null => Ok(py.None().into_bound(py)),
            SerdeValue::Bool(val) => Ok(val.into_py(py).into_bound(py)),
            SerdeValue::I64(val) => Ok(val.into_py(py).into_bound(py)),
            SerdeValue::F64(val) => Ok(val.into_py(py).into_bound(py)),
            SerdeValue::String(val) => Ok(val.into_py(py).into_bound(py)),
            SerdeValue::List(values) => {
                let list = PyList::empty_bound(py);
                for value in values {
                    list.append(value.to_python(py)?)?;
                }
                Ok(list.into_any())
            }
            SerdeValue::Dict(map) => {
                let dict = PyDict::new_bound(py);
                for (key, value) in map {
                    dict.set_item(key, value.to_python(py)?)?;
                }
                Ok(dict.into_any())
            }
        }
    }
}

/// Convert a Python dict-like object to bincode bytes
pub fn python_to_bincode(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let value = SerdeValue::from_python(obj)?;
    bincode::serialize(&value).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Bincode serialization failed: {}", e))
    })
}

/// Convert bincode bytes to a Python object
pub fn bincode_to_python<'py>(py: Python<'py>, bytes: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    let value: SerdeValue = bincode::deserialize(bytes).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Bincode deserialization failed: {}", e))
    })?;
    value.to_python(py)
}

/// Helper to serialize a Python dataclass instance to bincode
///
/// Extracts all fields from the dataclass instance into a dict and serializes
pub fn dataclass_to_bincode(obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    // Get the __dict__ attribute which contains all fields
    let dict = obj.getattr("__dict__")?;
    python_to_bincode(&dict)
}

/// Helper to deserialize bincode bytes into a Python dataclass
///
/// Creates a dict from the bytes and then constructs the dataclass
pub fn bincode_to_dataclass<'py>(
    py: Python<'py>,
    class: &Bound<'py, PyAny>,
    bytes: &[u8],
) -> PyResult<Bound<'py, PyAny>> {
    let dict = bincode_to_python(py, bytes)?;
    let dict_ref = dict.downcast::<PyDict>()?;

    // Call the dataclass constructor with **kwargs
    class.call((), Some(dict_ref))
}

/// Python-exposed function to convert a Python object to bincode bytes
#[pyfunction]
pub fn python_to_bincode_py<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let bytes = python_to_bincode(obj)?;
    Ok(PyBytes::new_bound(obj.py(), &bytes))
}

/// Python-exposed function to convert bincode bytes to a Python object
#[pyfunction]
pub fn bincode_to_python_py<'py>(py: Python<'py>, bytes: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    bincode_to_python(py, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_value_roundtrip() {
        let value = SerdeValue::Dict(
            vec![
                ("name".to_string(), SerdeValue::String("Alice".to_string())),
                ("age".to_string(), SerdeValue::I64(30)),
                ("active".to_string(), SerdeValue::Bool(true)),
            ]
            .into_iter()
            .collect(),
        );

        let bytes = bincode::serialize(&value).unwrap();
        let deserialized: SerdeValue = bincode::deserialize(&bytes).unwrap();

        match deserialized {
            SerdeValue::Dict(map) => {
                assert_eq!(map.len(), 3);
                assert!(matches!(map.get("name"), Some(SerdeValue::String(s)) if s == "Alice"));
                assert!(matches!(map.get("age"), Some(SerdeValue::I64(30))));
                assert!(matches!(map.get("active"), Some(SerdeValue::Bool(true))));
            }
            _ => panic!("Expected Dict variant"),
        }
    }
}
