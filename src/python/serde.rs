//! Serialization bridge between Python and Rust using MessagePack.
//!
//! This module provides utilities to convert between Python objects and MessagePack-serialized bytes.

#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use pyo3::IntoPyObject;
use serde::{Deserialize, Serialize};

/// A generic value that can be serialized/deserialized between Python and Rust.
///
/// This acts as an intermediate representation that can be converted to/from
/// Python objects and serialized with MessagePack (not bincode).
///
/// Note: We use Vec<(String, SerdeValue)> for Dict to maintain insertion order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerdeValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    List(Vec<SerdeValue>),
    Dict(Vec<(String, SerdeValue)>),
}

impl SerdeValue {
    /// Convert a Python object to a SerdeValue
    ///
    /// Note: Order matters! Python bools are also ints, so we must check bool first.
    pub fn from_python(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        // Check None first
        if obj.is_none() {
            return Ok(SerdeValue::Null);
        }

        // Check for container types before primitives
        if let Ok(dict) = obj.downcast::<PyDict>() {
            let mut entries = Vec::new();
            for (key, value) in dict.iter() {
                let key_str = key.extract::<String>()?;
                entries.push((key_str, SerdeValue::from_python(&value)?));
            }
            return Ok(SerdeValue::Dict(entries));
        }

        if let Ok(list) = obj.downcast::<PyList>() {
            let mut values = Vec::new();
            for item in list.iter() {
                values.push(SerdeValue::from_python(&item)?);
            }
            return Ok(SerdeValue::List(values));
        }

        // Check bool BEFORE int (Python bools are subclass of int)
        if obj.is_instance_of::<pyo3::types::PyBool>() {
            return Ok(SerdeValue::Bool(obj.extract::<bool>()?));
        }

        // Check string before numeric types (to avoid conversion issues)
        if let Ok(val) = obj.extract::<String>() {
            return Ok(SerdeValue::String(val));
        }

        // Try integer
        if let Ok(val) = obj.extract::<i64>() {
            return Ok(SerdeValue::I64(val));
        }

        // Try float
        if let Ok(val) = obj.extract::<f64>() {
            return Ok(SerdeValue::F64(val));
        }

        // If nothing matched, error
        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Cannot convert Python type {} to SerdeValue",
            obj.get_type().name()?
        )))
    }

    /// Convert a SerdeValue to a Python object
    #[allow(deprecated)]
    pub fn to_python<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            SerdeValue::Null => Ok(py.None().into_bound(py)),
            SerdeValue::Bool(val) => Ok(val.to_object(py).into_bound(py)),
            SerdeValue::I64(val) => Ok(val.into_pyobject(py).unwrap().into_any()),
            SerdeValue::F64(val) => Ok(val.into_pyobject(py).unwrap().into_any()),
            SerdeValue::String(val) => Ok(val.into_pyobject(py).unwrap().into_any()),
            SerdeValue::List(values) => {
                let list = PyList::empty(py);
                for value in values {
                    list.append(value.to_python(py)?)?;
                }
                Ok(list.into_any())
            }
            SerdeValue::Dict(entries) => {
                let dict = PyDict::new(py);
                for (key, value) in entries {
                    dict.set_item(key, value.to_python(py)?)?;
                }
                Ok(dict.into_any())
            }
        }
    }
}

/// Convert Python dict directly to MessagePack bytes
///
/// This is used for Python-Rust interop where Rust expects a raw struct format.
/// This serializes the dict directly to MessagePack without any wrapper.
#[pyfunction]
pub fn python_to_msgpack_py<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    // Convert Python dict to rmpv::Value directly (preserves order and structure)
    if let Ok(_dict) = obj.downcast::<PyDict>() {
        let val = python_value_to_msgpack_value(obj)?;

        // Serialize the rmpv::Value directly to MessagePack bytes
        // Use rmpv's write_value to preserve the exact MessagePack structure
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, &val).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "MessagePack serialization failed: {}",
                e
            ))
        })?;

        Ok(PyBytes::new(obj.py(), &bytes))
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err("Expected a dict"))
    }
}

/// Convert Python value to rmpv::Value for direct MessagePack serialization
fn python_value_to_msgpack_value(obj: &Bound<'_, PyAny>) -> PyResult<rmpv::Value> {
    if obj.is_none() {
        Ok(rmpv::Value::Nil)
    } else if obj.is_instance_of::<pyo3::types::PyBool>() {
        Ok(rmpv::Value::Boolean(obj.extract::<bool>()?))
    } else if let Ok(val) = obj.extract::<i64>() {
        Ok(rmpv::Value::Integer(rmpv::Integer::from(val)))
    } else if let Ok(val) = obj.extract::<f64>() {
        Ok(rmpv::Value::F64(val))
    } else if let Ok(val) = obj.extract::<String>() {
        Ok(rmpv::Value::String(rmpv::Utf8String::from(val)))
    } else if let Ok(list) = obj.downcast::<PyList>() {
        let mut vec = Vec::new();
        for item in list {
            vec.push(python_value_to_msgpack_value(&item)?);
        }
        Ok(rmpv::Value::Array(vec))
    } else if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut vec = Vec::new();
        for (key, value) in dict {
            let key_val = python_value_to_msgpack_value(&key)?;
            let val = python_value_to_msgpack_value(&value)?;
            vec.push((key_val, val));
        }
        Ok(rmpv::Value::Map(vec))
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Unsupported Python type for MessagePack conversion: {}",
            obj.get_type().name()?
        )))
    }
}

/// Convert MessagePack bytes directly to Python dict without SerdeValue wrapper
#[pyfunction]
pub fn msgpack_to_python_py<'py>(py: Python<'py>, bytes: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    let value: rmpv::Value = rmp_serde::from_slice(bytes).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "MessagePack deserialization failed: {}",
            e
        ))
    })?;

    msgpack_value_to_python(py, &value)
}

/// Convert rmpv::Value to Python object
#[allow(deprecated)]
fn msgpack_value_to_python<'py>(
    py: Python<'py>,
    value: &rmpv::Value,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        rmpv::Value::Nil => Ok(py.None().into_bound(py)),
        rmpv::Value::Boolean(b) => Ok(b.to_object(py).into_bound(py)),
        rmpv::Value::Integer(i) => {
            if let Some(val) = i.as_i64() {
                Ok(val.into_pyobject(py).unwrap().into_any())
            } else if let Some(val) = i.as_u64() {
                Ok(val.into_pyobject(py).unwrap().into_any())
            } else {
                Err(pyo3::exceptions::PyValueError::new_err(
                    "Integer out of range",
                ))
            }
        }
        rmpv::Value::F32(f) => Ok((*f as f64).into_pyobject(py).unwrap().into_any()),
        rmpv::Value::F64(f) => Ok(f.into_pyobject(py).unwrap().into_any()),
        rmpv::Value::String(s) => Ok(s.as_str().into_pyobject(py).unwrap().into_any()),
        rmpv::Value::Binary(b) => Ok(PyBytes::new(py, b).into_any()),
        rmpv::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(msgpack_value_to_python(py, item)?)?;
            }
            Ok(list.into_any())
        }
        rmpv::Value::Map(map) => {
            let dict = PyDict::new(py);
            for (key, value) in map {
                let py_key = msgpack_value_to_python(py, key)?;
                let py_value = msgpack_value_to_python(py, value)?;
                dict.set_item(py_key, py_value)?;
            }
            Ok(dict.into_any())
        }
        rmpv::Value::Ext(_, _) => Err(pyo3::exceptions::PyValueError::new_err(
            "Extension types not supported",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_value_roundtrip() {
        let value = SerdeValue::Dict(vec![
            ("name".to_string(), SerdeValue::String("Alice".to_string())),
            ("age".to_string(), SerdeValue::I64(30)),
            ("active".to_string(), SerdeValue::Bool(true)),
        ]);

        let bytes = rmp_serde::to_vec(&value).unwrap();
        let deserialized: SerdeValue = rmp_serde::from_slice(&bytes).unwrap();

        match deserialized {
            SerdeValue::Dict(entries) => {
                assert_eq!(entries.len(), 3);
                assert!(entries.iter().any(
                    |(k, v)| k == "name" && matches!(v, SerdeValue::String(s) if s == "Alice")
                ));
                assert!(entries
                    .iter()
                    .any(|(k, v)| k == "age" && matches!(v, SerdeValue::I64(30))));
                assert!(entries
                    .iter()
                    .any(|(k, v)| k == "active" && matches!(v, SerdeValue::Bool(true))));
            }
            _ => panic!("Expected Dict variant"),
        }
    }
}
