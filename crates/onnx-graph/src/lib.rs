pub mod operators;
pub mod weights;
pub mod tensor;
mod node;
pub mod pytorch;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tensor::*;
use node::*;
use crate::weights::{WeightExternalOutputManager};

pub mod onnx {
    include!(concat!(env!("OUT_DIR"), "/onnx.rs"));
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bad input shape: {0}")]
    InputShapeError(Shape),
    #[error("Shape mismatch: {0} != {1}")]
    ShapeMismatchError(Shape, Shape),
    #[error("Incompatible shapes: {0}, {1}")]
    IncompatibleShapeError(Shape, Shape),
    #[error("DType mismatch: {0} != {1}")]
    DTypeMismatchError(DType, DType),
    #[error("Invalid input")]
    InvalidInputError,
    #[error("Invalid dtype")]
    UnsupportedDTypeError,
    #[error("Name conflict: {0}")]
    NameConflictError(String),
    #[error("No such tensor: {0}")]
    NoSuchTensorError(String),
    #[error("Unresolved dimension")]
    UnresolvedDimensionError,
    #[error("Invalid dtype")]
    InvalidDTypeError,
    #[error("Cannot resolve data")]
    CannotResolveDataError,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    CandleCoreError(#[from] candle_core::Error),
    #[error(transparent)]
    SafeTensorError(#[from] safetensors::SafeTensorError),
    #[error("Other error")]
    OtherError
}

pub enum WeightStorageStrategy {
    None,
    BinFile(PathBuf),
    EmbeddedData
}

impl WeightStorageStrategy {
    fn get_manager<'a>(&'a self) -> Result<Box<dyn WeightExternalOutputManager<'a> + 'a>, Error> {
        match self {
            WeightStorageStrategy::None => Ok(Box::new(weights::NullOutputManager::new())),
            WeightStorageStrategy::BinFile(path) => Ok(Box::new(weights::BinOutputManager::<'a>::new(path))),
            WeightStorageStrategy::EmbeddedData => Ok(Box::new(weights::EmbeddedOutputManager::<'a>::new())),
            _ => panic!()
        }
    }
}

pub fn build_proto(
    inputs: &[Arc<InputTensor>],
    outputs: &[(String, Arc<dyn Tensor>)],
    weight_storage: WeightStorageStrategy
) -> Result<onnx::ModelProto, Error> {
    
    // Get all nodes in graph
    let mut nodes = HashSet::new();
    for (_, tensor) in outputs {
        tensor.get_nodes(&mut nodes);
    }
    
    // Get requested node names
    let mut node_names: HashMap<&dyn Node, String> = HashMap::new();
    for node in &nodes {
        if let Some(name) = node.get_name() {
            if !node_names.contains_key(node) {
                node_names.insert(*node, name.to_string());
            }
        }
    }
    println!("Found {} nodes in graph", nodes.len());

    // Get all tensors in graph
    let mut tensors = HashSet::new();
    for (_, tensor) in outputs {
        tensors.insert(tensor.as_ref());
        tensor.get_sub_tensors(&mut tensors);
    }
    println!("Found {} tensors in graph", tensors.len());

    // Assign names to all tensors in graph
    let mut chosen_names: HashSet<String> = HashSet::new();
    let mut tensor_names: HashMap<&dyn Tensor, String> = HashMap::new();

    // Assign requested names
    for tensor in &tensors { 
        if let Some(name) = tensor.get_name() {
            let name = name.to_string();
            if chosen_names.contains(&name) {
                return Err(Error::NameConflictError(name));
            }
            chosen_names.insert(name.clone());
            tensor_names.insert(*tensor, name);
        }
    }
    for (name, tensor) in outputs {
        chosen_names.insert(name.clone());
        tensor_names.insert(tensor.as_ref(), name.clone());
    }
    // Assign remaining names
    let mut next_tensor_id = 0;
    for tensor in &tensors {
        if !tensor_names.contains_key(tensor) {
            let name = loop {
                let name = format!("tensor_{}", next_tensor_id);
                if !chosen_names.contains(&name) {
                    break name;
                }
                next_tensor_id += 1;
            };
            
            tensor_names.insert(*tensor, name.clone());
            chosen_names.insert(name);
            next_tensor_id += 1;
        }
    }

    // Gather tensor weights
    let mut data_manager = weight_storage.get_manager()?;
    for tensor in &tensors {
        tensor.gather_weights(data_manager.as_mut());
    }
    data_manager.finalize_tensor_data();
    
    // Find tensors that are not input or output, and add initializer sections
    let mut tensors_to_enumerate = vec![];
    for tensor in &tensors {
        // Check if tensor is not in inputs and outputs
        if !inputs.iter().any(|t| (t.as_ref() as &dyn Tensor) == *tensor) && !outputs.iter().any(|(_, t)| t.as_ref() == *tensor) {
            tensors_to_enumerate.push(*tensor);
        }
    }
    
    // Generate initializer blocks
    let mut initializers = vec![];
    for tensor in tensors {
        if let Some(initializer) = tensor.get_initializer(tensor_names[&tensor].clone(), data_manager.as_mut())? {
            initializers.push(initializer);
        }
    }
    
    let graph = onnx::GraphProto {
        name: String::new(),
        node: nodes.iter().map(|node| node.to_node_proto(node_names.get(node).map(|name| name.clone()), &tensor_names)).collect(),
        initializer: initializers,
        doc_string: String::new(),
        input: inputs.iter().map(|tensor| tensor.to_value_info_proto(tensor_names[&(tensor.as_ref() as &dyn Tensor)].clone())).collect(),
        output: outputs.iter().map(|(name, tensor)| tensor.to_value_info_proto(name.to_string())).collect(),
        value_info: tensors_to_enumerate.iter().map(|tensor| tensor.to_value_info_proto(tensor_names.get(tensor).unwrap().to_string())).collect(),
        metadata_props: vec![],
        .. Default::default()
    };

    let own_version = env!("CARGO_PKG_VERSION").to_string();
    Ok(onnx::ModelProto {
        ir_version: onnx::Version::IrVersion2024325 as i64,
        opset_import: vec![],
        producer_version: own_version,
        domain: String::new(),
        model_version: 0,
        doc_string: String::new(),
        graph: Some(graph),
        metadata_props: vec![],
        training_info: vec![],
        functions: vec![],
        .. Default::default()
    })
}